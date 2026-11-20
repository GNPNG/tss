// The editor owns the helper's stdin. Its closure terminates the listener.
package main

import (
	"bufio"
	"bytes"
	"crypto/rand"
	"crypto/subtle"
	"crypto/tls"
	"crypto/x509"
	"embed"
	"encoding/base64"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log"
	"mime"
	"net"
	"net/http"
	"net/url"
	"os"
	"strconv"
	"strings"
	"sync"
	"time"
)

//go:embed surface/*
var assets embed.FS

type config struct {
	Bind, Host, Cert, Key string
	Port                  int
}
type sample struct {
	Time           *float64 `json:"time"`
	PointerID      *int64   `json:"pointerId"`
	PointerType    string   `json:"pointerType"`
	Phase          string   `json:"phase"`
	X              *float64 `json:"x"`
	Y              *float64 `json:"y"`
	Pressure       *float64 `json:"pressure"`
	PressureSource *string  `json:"pressureSource"`
	RawForce       *float64 `json:"rawForce"`
	TiltX          *float64 `json:"tiltX"`
	TiltY          *float64 `json:"tiltY"`
}

func in(v *float64, lo, hi float64) bool { return v != nil && *v >= lo && *v <= hi }
func (s sample) valid() bool {
	return in(s.Time, 0, 1e10) && s.PointerID != nil && *s.PointerID >= 0 && *s.PointerID <= 9007199254740991 && in(s.X, 0, 1) && in(s.Y, 0, 1) && (s.Pressure == nil || in(s.Pressure, 0, 1)) && (s.PressureSource == nil || *s.PressureSource == "webkitForce" || *s.PressureSource == "pointer") && (s.RawForce == nil || in(s.RawForce, 0, 1e6)) && (s.TiltX == nil || in(s.TiltX, -90, 90)) && (s.TiltY == nil || in(s.TiltY, -90, 90)) && (s.PointerType == "mouse" || s.PointerType == "pen" || s.PointerType == "touch") && (s.Phase == "down" || s.Phase == "move" || s.Phase == "up" || s.Phase == "cancel")
}

type batch struct {
	Sequence uint64   `json:"sequence"`
	Samples  []sample `json:"samples"`
}
type event struct {
	Kind    string   `json:"kind"`
	Source  string   `json:"source,omitempty"`
	Name    string   `json:"name,omitempty"`
	Samples []sample `json:"samples,omitempty"`
	URL     string   `json:"url,omitempty"`
	Secret  string   `json:"secret,omitempty"`
	Error   string   `json:"error,omitempty"`
}
type session struct {
	source string
	last   time.Time
	seq    uint64
}
type hub struct {
	mu                sync.Mutex
	secret, authority string
	sessions          map[string]*session
	emit              func(event)
	threshold         time.Time
	used              int
}

func token() string {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		panic(err)
	}
	return base64.RawURLEncoding.EncodeToString(b)
}
func newHub(host string, sink func(event)) *hub {
	return &hub{authority: host, secret: token(), sessions: make(map[string]*session), emit: sink}
}
func httpsAuthority(hostPort string) string {
	hostPort = strings.ToLower(hostPort)
	_, port, err := net.SplitHostPort(hostPort)
	if err == nil && port == "443" {
		return strings.TrimSuffix(hostPort, ":443")
	}
	return hostPort
}
func (h *hub) expire(now time.Time) {
	h.mu.Lock()
	defer h.mu.Unlock()
	for k, s := range h.sessions {
		if now.Sub(s.last) > 15*time.Second {
			delete(h.sessions, k)
			h.emit(event{Kind: "left", Source: s.source})
		}
	}
}
func (h *hub) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Cache-Control", "no-store")
	w.Header().Set("X-Content-Type-Options", "nosniff")
	w.Header().Set("Referrer-Policy", "no-referrer")
	if httpsAuthority(r.Host) != h.authority {
		http.Error(w, "Incorrect host", 421)
		return
	}
	if r.Method == "GET" {
		files := map[string]string{"/": "index.html", "/client.mjs": "client.mjs", "/gesture-input.mjs": "gesture-input.mjs", "/surface.css": "surface.css"}
		name, ok := files[r.URL.Path]
		if !ok {
			http.NotFound(w, r)
			return
		}
		data, err := assets.ReadFile("surface/" + name)
		if err != nil {
			http.NotFound(w, r)
			return
		}
		typ := "text/javascript; charset=utf-8"
		if name == "index.html" {
			typ = "text/html; charset=utf-8"
		} else if name == "surface.css" {
			typ = "text/css; charset=utf-8"
		}
		w.Header().Set("Content-Type", typ)
		w.Header().Set("Content-Security-Policy", "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'")
		w.Write(data)
		return
	}
	if r.Method != "POST" || (r.URL.Path != "/pair" && r.URL.Path != "/gestures" && r.URL.Path != "/pulse" && r.URL.Path != "/unpair") {
		http.NotFound(w, r)
		return
	}
	typ, _, _ := mime.ParseMediaType(r.Header.Get("Content-Type"))
	if r.Header.Get("Origin") != ("https://"+h.authority) || typ != "application/json" {
		http.Error(w, "Origin or type", 403)
		return
	}
	auth, ok := strings.CutPrefix(r.Header.Get("Authorization"), "Bearer ")
	if !ok || len(auth) != 43 {
		http.Error(w, "Missing pairing", 401)
		return
	}
	b, err := io.ReadAll(http.MaxBytesReader(w, r.Body, 16384))
	if err != nil {
		http.Error(w, "Event body exceeds limit", 413)
		return
	}
	h.mu.Lock()
	defer h.mu.Unlock()
	now := time.Now()
	if r.URL.Path == "/pair" {
		if subtle.ConstantTimeCompare([]byte(auth), []byte(h.secret)) != 1 {
			http.Error(w, "Incorrect session secret", 401)
			return
		}
		if len(h.sessions) >= 1024 {
			http.Error(w, "Session capacity", 503)
			return
		}
		var p struct {
			Name string `json:"name"`
		}
		if json.Unmarshal(b, &p) != nil || len(strings.TrimSpace(p.Name)) < 1 || len(p.Name) > 80 {
			http.Error(w, "Invalid performer name", 400)
			return
		}
		id, key := token()[:16], token()
		h.sessions[key] = &session{source: id, last: now}
		h.emit(event{Kind: "joined", Source: id, Name: strings.TrimSpace(p.Name)})
		w.Header().Set("Content-Type", "application/json")
		json.NewEncoder(w).Encode(map[string]string{"source": id, "token": key})
		return
	}
	s, ok := h.sessions[auth]
	if !ok {
		http.Error(w, "Pair again", 401)
		return
	}
	s.last = now
	if r.URL.Path == "/unpair" {
		delete(h.sessions, auth)
		h.emit(event{Kind: "left", Source: s.source})
		w.WriteHeader(204)
		return
	}
	if r.URL.Path == "/pulse" {
		w.WriteHeader(204)
		return
	}
	var input batch
	dec := json.NewDecoder(bytes.NewReader(b))
	if dec.Decode(&input) != nil || dec.Decode(new(any)) != io.EOF || len(input.Samples) == 0 || len(input.Samples) > 32 {
		http.Error(w, "Invalid input batch", 400)
		return
	}
	if input.Sequence != s.seq+1 {
		http.Error(w, "Out-of-order input; re-pair", 409)
		return
	}
	for _, x := range input.Samples {
		if !x.valid() {
			http.Error(w, "Invalid gesture", 400)
			return
		}
	}
	if now.Sub(h.threshold) >= time.Second {
		h.threshold = now
		h.used = 0
	}
	if h.used+len(input.Samples) > 5000 {
		http.Error(w, "Input rate exceeded", 429)
		return
	}
	h.used += len(input.Samples)
	s.seq = input.Sequence
	h.emit(event{Kind: "batch", Source: s.source, Samples: input.Samples})
	w.WriteHeader(204)
}
func run(in io.Reader, out io.Writer, proof bool) error {
	var c config
	reader := bufio.NewReaderSize(in, 16384)
	line, err := reader.ReadSlice('\n')
	if err != nil {
		return errors.New("Missing performer listener configuration")
	}
	if err = json.Unmarshal(line, &c); err != nil {
		return err
	}
	if net.ParseIP(c.Bind) == nil || c.Port < 0 || c.Port > 65535 || len(c.Host) == 0 || strings.ContainsAny(c.Host, "/\\?#@% \t\r\n") {
		return errors.New("Specify a local bind IP, certificate hostname and valid port")
	}
	var pair tls.Certificate
	if proof {
		if c.Bind != "127.0.0.1" || c.Host != "127.0.0.1" || c.Port != 0 {
			return errors.New("Verification accepts an ephemeral IPv4 loopback endpoint only")
		}
		pair, err = localProofCertificate()
	} else {
		pair, err = tls.LoadX509KeyPair(c.Cert, c.Key)
	}
	if err != nil {
		return fmt.Errorf("Read TLS certificate and key: %w", err)
	}
	leaf, err := x509.ParseCertificate(pair.Certificate[0])
	if err != nil {
		return err
	}
	if err = leaf.VerifyHostname(c.Host); err != nil {
		return fmt.Errorf("Certificate hostname: %w", err)
	}
	if time.Now().Before(leaf.NotBefore) || time.Now().After(leaf.NotAfter) {
		return errors.New("Controller certificate is outside its validity period")
	}
	ln, err := net.Listen("tcp", net.JoinHostPort(c.Bind, strconv.Itoa(c.Port)))
	if err != nil {
		return err
	}
	defer ln.Close()
	authority := net.JoinHostPort(c.Host, strconv.Itoa(ln.Addr().(*net.TCPAddr).Port))
	origin := url.URL{Scheme: "https", Host: httpsAuthority(authority)}
	var mu sync.Mutex
	jsonOut := json.NewEncoder(out)
	write := func(e event) { mu.Lock(); defer mu.Unlock(); _ = jsonOut.Encode(e) }
	h := newHub(origin.Host, write)
	srv := &http.Server{Handler: h, TLSConfig: &tls.Config{MinVersion: tls.VersionTLS12, Certificates: []tls.Certificate{pair}}, ReadHeaderTimeout: 5 * time.Second, ReadTimeout: 8 * time.Second, WriteTimeout: 8 * time.Second, IdleTimeout: 20 * time.Second, MaxHeaderBytes: 8192, ErrorLog: log.New(io.Discard, "", 0)}
	stopped := make(chan struct{})
	defer close(stopped)
	if proof {
		go func() {
			if e := exerciseLoopback(reader, origin.String(), h.secret, leaf, write); e != nil {
				write(event{Kind: "proof-error", Error: e.Error()})
			}
			srv.Close()
		}()
	} else {
		go func() { _, _ = io.Copy(io.Discard, reader); srv.Close() }()
	}
	go func() {
		ticker := time.NewTicker(time.Second)
		defer ticker.Stop()
		for {
			select {
			case t := <-ticker.C:
				h.expire(t)
			case <-stopped:
				return
			}
		}
	}()
	write(event{Kind: "ready", URL: origin.String(), Secret: h.secret})
	err = srv.ServeTLS(ln, "", "")
	if errors.Is(err, http.ErrServerClosed) {
		return nil
	}
	return err
}
func main() {
	if err := run(os.Stdin, os.Stdout, len(os.Args) == 2 && os.Args[1] == "--verify-loopback"); err != nil {
		json.NewEncoder(os.Stdout).Encode(event{Kind: "error", Error: err.Error()})
		os.Exit(1)
	}
}
