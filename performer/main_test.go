package main

import (
	"bytes"
	"crypto/tls"
	"crypto/x509"
	"encoding/json"
	"io"
	"net/http"
	"net/http/httptest"
	"slices"
	"strings"
	"sync"
	"testing"
	"time"
)

func TestHTTPSAuthorityMatchesBrowserOrigins(t *testing.T) {
	for input, expected := range map[string]string{
		"DECK.example:443": "deck.example", "[::1]:443": "[::1]", "[::1]:8443": "[::1]:8443",
		"deck.example:8443": "deck.example:8443", "deck.example:8443:443": "deck.example:8443:443",
	} {
		if got := httpsAuthority(input); got != expected {
			t.Errorf("httpsAuthority(%q) = %q, want %q", input, got, expected)
		}
	}
}

func TestTrustedPairingAndIsolation(t *testing.T) {
	var mu sync.Mutex
	var events []event
	h := newHub("", func(e event) { mu.Lock(); defer mu.Unlock(); events = append(events, e) })
	srv := httptest.NewUnstartedServer(h)
	h.authority = srv.Listener.Addr().String()
	srv.StartTLS()
	defer srv.Close()
	roots := x509.NewCertPool()
	roots.AddCert(srv.Certificate())
	client := &http.Client{Transport: &http.Transport{TLSClientConfig: &tls.Config{RootCAs: roots, MinVersion: tls.VersionTLS12}}}
	post := func(path, key, origin string, body string) (int, map[string]string) {
		t.Helper()
		req, err := http.NewRequest("POST", srv.URL+path, strings.NewReader(body))
		if err != nil {
			t.Fatal(err)
		}
		req.Header.Set("Origin", origin)
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Authorization", "Bearer "+key)
		res, err := client.Do(req)
		if err != nil {
			t.Fatal(err)
		}
		defer res.Body.Close()
		var reply map[string]string
		_ = json.NewDecoder(res.Body).Decode(&reply)
		return res.StatusCode, reply
	}
	pair := func(name string) map[string]string {
		code, reply := post("/pair", h.secret, srv.URL, `{"name":"`+name+`"}`)
		if code != 200 {
			t.Fatal(code)
		}
		return reply
	}
	if code, _ := post("/pair", h.secret, "https://wrong.invalid", `{"name":"bad"}`); code != 403 {
		t.Fatal(code)
	}
	if code, _ := post("/pair", strings.Repeat("x", 43), srv.URL, `{"name":"bad"}`); code != 401 {
		t.Fatal(code)
	}
	m, p := pair("Force Touch"), pair("Pencil")
	if m["source"] == p["source"] || m["token"] == p["token"] {
		t.Fatal("device identities collided")
	}
	mouse := `{"sequence":1,"samples":[{"time":0.02,"pointerId":1,"pointerType":"mouse","phase":"move","x":0.3,"y":0.4,"pressure":0.73,"pressureSource":"webkitForce"}]}`
	if code, _ := post("/gestures", m["token"], srv.URL, mouse); code != 204 {
		t.Fatal(code)
	}
	if code, _ := post("/gestures", m["token"], srv.URL, mouse); code != 409 {
		t.Fatal("duplicate sequence", code)
	}
	pen := `{"sequence":1,"samples":[{"time":3,"pointerId":1,"pointerType":"pen","phase":"down","x":0.5,"y":0.7,"pressure":0.5,"tiltX":10}]}`
	if code, _ := post("/gestures", p["token"], srv.URL, pen); code != 204 {
		t.Fatal(code)
	}
	if code, _ := post("/gestures", p["token"], srv.URL, `{"sequence":2,"samples":[{"x":0.1}]}`); code != 400 {
		t.Fatal(code)
	}
	if code, _ := post("/unpair", m["token"], srv.URL, "{}"); code != 204 {
		t.Fatal(code)
	}
	if code, _ := post("/pulse", m["token"], srv.URL, "{}"); code != 401 {
		t.Fatal(code)
	}
	h.expire(time.Now().Add(time.Minute))
	mu.Lock()
	got := slices.Clone(events)
	mu.Unlock()
	var ids []string
	for _, e := range got {
		ids = append(ids, e.Source+":"+e.Kind)
	}
	expected := []string{m["source"] + ":joined", p["source"] + ":joined", m["source"] + ":batch", p["source"] + ":batch", m["source"] + ":left", p["source"] + ":left"}
	if !slices.Equal(ids, expected) {
		t.Fatal(ids)
	}
	if *got[2].Samples[0].Pressure != .73 || got[3].Samples[0].PointerType != "pen" {
		t.Fatal("input data lost")
	}
	for _, suffix := range []string{"/", "/gesture-input.mjs", "/client.mjs", "/surface.css"} {
		r, e := client.Get(srv.URL + suffix)
		if e != nil {
			t.Fatal(e)
		}
		b, _ := io.ReadAll(r.Body)
		r.Body.Close()
		if r.StatusCode != 200 || bytes.Contains(b, []byte(h.secret)) {
			t.Fatal(suffix)
		}
	}
}

func TestLoopbackDesktopFixture(t *testing.T) {
	input, commands := io.Pipe()
	events, output := io.Pipe()
	done := make(chan error, 1)
	go func() { done <- run(input, output, true); output.Close() }()
	defer commands.Close()
	io.WriteString(commands, "{\"bind\":\"127.0.0.1\",\"host\":\"127.0.0.1\",\"port\":0}\n")
	dec := json.NewDecoder(events)
	var ready event
	if err := dec.Decode(&ready); err != nil || ready.Kind != "ready" {
		t.Fatal("startup", err)
	}
	for _, stage := range []struct {
		op, end string
		count   int
	}{{"join", "joined", 2}, {"hold", "held", 2}, {"release", "released", 4}} {
		io.WriteString(commands, stage.op+"\n")
		n := 0
		for {
			var got event
			if err := dec.Decode(&got); err != nil {
				t.Fatal(err)
			}
			if got.Kind == "proof-error" {
				t.Fatal(got.Error)
			}
			if got.Kind == "proof-step" {
				if got.Name != stage.end || n != stage.count {
					t.Fatal(got.Name, n)
				}
				break
			}
			n++
		}
	}
	commands.Close()
	if err := <-done; err != nil {
		t.Fatal(err)
	}
	events.Close()
}
