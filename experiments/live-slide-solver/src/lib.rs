//! One persistent Basilisk process per sealed fluid cell. All calls are blocking;
//! call this interface on a solver worker thread, outside the presentation loop.
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

#[cfg(feature = "cimg-optics")]
pub mod optics;

#[derive(Clone, Copy, Debug)]
pub enum Slide {
    Violet,
    Cyan,
}

/// Tangential motion of the confining glass during a simulation step.
#[derive(Clone, Copy, Debug)]
pub enum PlateMotion {
    AtRest,
    Driven,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Drive {
    /// Contact in normalized full-slide coordinates, top-left origin.
    pub x: f64,
    pub y: f64,
    /// Displacement in slide widths/heights over the next 1/24 simulated second.
    pub dx: f64,
    pub dy: f64,
    /// Local moving-plate velocity multiplier, 0 for a released contact.
    pub drive: f64,
}

/// A normal load on one glass cell. Distances are fractions of the full slide width.
#[derive(Clone, Copy, Debug, Default)]
pub struct Squeeze {
    pub center: [f64; 2],
    pub radius: f64,
    pub load: f64,
}

impl Squeeze {
    fn validate(self) -> io::Result<()> {
        if self.center.into_iter().chain([self.radius, self.load]).all(|v| (0.0..=1.).contains(&v))
            && (self.load == 0. || self.radius > 0.)
        {
            return Ok(());
        }
        Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid squeeze contact"))
    }
}

#[derive(Clone, Debug)]
pub struct Field {
    pub tick: u64,
    pub seconds: f64,
    pub width: usize,
    pub height: usize,
    /// Conserved dyed volume divided by full cell volume, before rasterizing.
    pub phase_fraction: f64,
    /// Uncolored linear coverage, row major, first row at top.
    pub coverage: Vec<u8>,
}

#[derive(Clone)]
pub struct Cancellation(Arc<Mutex<Child>>);

impl Cancellation {
    /// Interrupt an in-progress pipe read by terminating the registered child.
    /// Its owner waits for exit before disposing of the process handle.
    pub fn cancel(&self) {
        let mut child = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if !matches!(child.try_wait(), Ok(Some(_))) {
            let _ = child.kill();
        }
    }
}

pub struct Cell {
    child: Cancellation,
    commands: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    width: usize,
    next_tick: u64,
}

impl Cell {
    pub fn start(
        executable: &Path,
        slide: Slide,
        seed: u32,
        level: u32,
    ) -> io::Result<(Self, Field)> {
        Self::start_registered(executable, slide, seed, level, |_| {})
    }

    /// Register the process immediately after spawn, before its first blocking read.
    pub fn start_registered(
        executable: &Path,
        slide: Slide,
        seed: u32,
        level: u32,
        register: impl FnOnce(Cancellation),
    ) -> io::Result<(Self, Field)> {
        Self::start_registered_with_motion(
            executable, slide, seed, level, PlateMotion::AtRest, register,
        )
    }

    pub fn start_registered_with_motion(
        executable: &Path,
        slide: Slide,
        seed: u32,
        level: u32,
        motion: PlateMotion,
        register: impl FnOnce(Cancellation),
    ) -> io::Result<(Self, Field)> {
        if !(5..=8).contains(&level) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "grid level outside 5..=8",
            ));
        }
        let name = match slide {
            Slide::Violet => "violet",
            Slide::Cyan => "cyan",
        };
        let mode = match motion {
            PlateMotion::AtRest => "--live",
            PlateMotion::Driven => "--live-driven",
        };
        let mut child = Command::new(executable)
            .args([mode, name])
            .arg(seed.to_string())
            .arg(level.to_string())
            .env("OMP_NUM_THREADS", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()?;
        let mut cell = Self {
            commands: child.stdin.take(),
            output: BufReader::new(child.stdout.take().expect("stdout piped")),
            child: Cancellation(Arc::new(Mutex::new(child))),
            width: 1 << level,
            next_tick: 0,
        };
        register(cell.child.clone());
        let first = cell.read_field()?;
        Ok((cell, first))
    }

    pub fn step(&mut self, drive: Drive) -> io::Result<Field> {
        self.advance(drive, None)
    }

    pub fn step_with_pressure(&mut self, drive: Drive, pressure: Squeeze) -> io::Result<Field> {
        self.advance(drive, Some(pressure))
    }

    pub fn press(&mut self, pressure: Squeeze) -> io::Result<Field> {
        pressure.validate()?;
        let commands = self.commands.as_mut().ok_or(io::ErrorKind::BrokenPipe)?;
        let [x, y] = pressure.center;
        writeln!(commands, "PRESS {x} {y} {} {}", pressure.radius, pressure.load)?;
        commands.flush()?;
        self.read_field()
    }

    fn advance(&mut self, drive: Drive, pressure: Option<Squeeze>) -> io::Result<Field> {
        if ![drive.x, drive.y, drive.drive]
            .iter()
            .all(|v| (0.0..=1.0).contains(v))
            || ![drive.dx, drive.dy]
                .iter()
                .all(|v| (-1.0..=1.0).contains(v))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid plate contact",
            ));
        }
        let commands = self.commands.as_mut().ok_or(io::ErrorKind::BrokenPipe)?;
        let Drive { x, y, dx, dy, drive } = drive;
        if let Some(p) = pressure {
            p.validate()?;
            let [px, py] = p.center;
            writeln!(commands, "STEP {x} {y} {dx} {dy} {drive} {px} {py} {} {}", p.radius, p.load)?;
        } else {
            writeln!(commands, "STEP {x} {y} {dx} {dy} {drive}")?;
        }
        commands.flush()?;
        self.read_field()
    }

    fn read_field(&mut self) -> io::Result<Field> {
        fn invalid() -> io::Error {
            io::Error::new(io::ErrorKind::InvalidData, "invalid Basilisk field header")
        }
        let mut header = String::new();
        self.output.by_ref().take(192).read_line(&mut header)?;
        if !header.ends_with('\n') {
            return Err(invalid());
        }
        let mut items = header.split_whitespace();
        let mut item = || items.next().ok_or_else(invalid);
        if item()? != "TSSVOF1" {
            return Err(invalid());
        }
        let tick = item()?.parse::<u64>().map_err(|_| invalid())?;
        let seconds = item()?.parse::<f64>().map_err(|_| invalid())?;
        let width = item()?.parse::<usize>().map_err(|_| invalid())?;
        let height = item()?.parse::<usize>().map_err(|_| invalid())?;
        let phase_fraction = item()?.parse::<f64>().map_err(|_| invalid())?;
        if items.next().is_some()
            || tick != self.next_tick
            || !seconds.is_finite()
            || seconds < 0.
            || width != self.width
            || height != self.width
            || !(0.0..=1.0).contains(&phase_fraction)
        {
            return Err(invalid());
        }
        let mut coverage = vec![0; width * height];
        self.output.read_exact(&mut coverage)?;
        self.next_tick += 1;
        Ok(Field {
            tick,
            seconds,
            width,
            height,
            phase_fraction,
            coverage,
        })
    }

    pub fn finish(mut self) -> io::Result<()> {
        if let Some(mut commands) = self.commands.take() {
            writeln!(commands, "QUIT")?;
        }
        if self
            .child
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .wait()?
            .success()
        {
            Ok(())
        } else {
            Err(io::Error::other("Basilisk worker failed"))
        }
    }
}

impl Drop for Cell {
    fn drop(&mut self) {
        self.commands.take();
        self.child.cancel();
        let _ = self
            .child
            .0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .wait();
    }
}
