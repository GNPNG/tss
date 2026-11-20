// An explicit desktop verification flag exercises the normal HTTP handler over
// loopback. Only this client trusts its ephemeral, in-memory test certificate.
package main

import (
	"bufio"
	"bytes"
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/json"
	"fmt"
	"io"
	"math/big"
	"net"
	"net/http"
	"time"
)

func localProofCertificate() (tls.Certificate, error) {
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		return tls.Certificate{}, err
	}
	now := time.Now()
	template := &x509.Certificate{SerialNumber: big.NewInt(1), Subject: pkix.Name{CommonName: "TSS loopback fixture"}, IPAddresses: []net.IP{net.ParseIP("127.0.0.1")}, NotBefore: now.Add(-time.Minute), NotAfter: now.Add(time.Hour), KeyUsage: x509.KeyUsageDigitalSignature, ExtKeyUsage: []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth}}
	der, err := x509.CreateCertificate(rand.Reader, template, template, &key.PublicKey, key)
	if err != nil {
		return tls.Certificate{}, err
	}
	return tls.Certificate{Certificate: [][]byte{der}, PrivateKey: key}, nil
}
func exerciseLoopback(input *bufio.Reader, origin, secret string, leaf *x509.Certificate, emit func(event)) error {
	pool := x509.NewCertPool()
	pool.AddCert(leaf)
	client := http.Client{Timeout: 3 * time.Second, Transport: &http.Transport{TLSClientConfig: &tls.Config{RootCAs: pool, MinVersion: tls.VersionTLS12}}}
	defer client.CloseIdleConnections()
	post := func(route, auth string, body any) (map[string]string, error) {
		data, err := json.Marshal(body)
		if err != nil {
			return nil, err
		}
		req, err := http.NewRequest("POST", origin+"/"+route, bytes.NewReader(data))
		if err != nil {
			return nil, err
		}
		req.Header.Set("Origin", origin)
		req.Header.Set("Content-Type", "application/json")
		req.Header.Set("Authorization", "Bearer "+auth)
		res, err := client.Do(req)
		if err != nil {
			return nil, err
		}
		defer res.Body.Close()
		if res.StatusCode < 200 || res.StatusCode > 299 {
			return nil, fmt.Errorf("%s returned %d", route, res.StatusCode)
		}
		var out map[string]string
		if res.StatusCode == 200 {
			err = json.NewDecoder(res.Body).Decode(&out)
		}
		return out, err
	}
	keys := make(map[string]string)
	for {
		line, err := input.ReadString('\n')
		if err == io.EOF {
			return nil
		}
		if err != nil {
			return err
		}
		switch line {
		case "join\n":
			for _, s := range []string{"Trackpad", "Pencil"} {
				p, e := post("pair", secret, map[string]string{"name": "Proof " + s})
				if e != nil {
					return e
				}
				keys[s] = p["token"]
			}
			emit(event{Kind: "proof-step", Name: "joined"})
		case "hold\n", "release\n":
			down := line == "hold\n"
			seq := 2
			if down {
				seq = 1
			}
			for _, name := range []string{"Trackpad", "Pencil"} {
				device, source := "mouse", "webkitForce"
				if name == "Pencil" {
					device = "pen"
					source = "pointer"
				}
				inputSample := func(phase string, x, y, pressure, t float64) map[string]any {
					return map[string]any{"pointerType": device, "pointerId": 1, "phase": phase, "x": x, "y": y, "pressure": pressure, "pressureSource": source, "time": t}
				}
				var samples []map[string]any
				if down {
					samples = []map[string]any{inputSample("down", .4, .3, 0, 1), inputSample("move", .61, .63, .75, 1.04)}
				} else {
					samples = []map[string]any{inputSample("up", .61, .63, 0, 1.12)}
				}
				if _, e := post("gestures", keys[name], map[string]any{"sequence": seq, "samples": samples}); e != nil {
					return e
				}
			}
			if !down {
				for _, k := range keys {
					if _, e := post("unpair", k, map[string]any{}); e != nil {
						return e
					}
				}
			}
			label := "released"
			if down {
				label = "held"
			}
			emit(event{Kind: "proof-step", Name: label})
		default:
			return fmt.Errorf("Unknown verification step")
		}
	}
}
