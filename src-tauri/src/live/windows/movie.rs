use super::*;
use crate::live::movie::Request;
pub(super) use crate::live::movie_render::{render, Job};
use std::sync::atomic::AtomicBool;

impl Host {
    pub async fn export_movie(
        &self,
        paths: (PathBuf, PathBuf),
        request: Request,
        cancel: Arc<AtomicBool>,
    ) -> Result<(), String> {
        self.request(move |reply| {
            Command::Movie(
                Job {
                    request,
                    paths,
                    cancel,
                },
                reply,
            )
        })
        .await
    }
}
