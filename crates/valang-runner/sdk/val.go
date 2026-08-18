// The SDK a Go handler is given.
//
// Verification already happened, in Rust, before this process started. There is
// one verifier and every language talks to it the same way: the runner hands
// the result in on stdin and this package shapes it.

package val

import (
	"encoding/json"
	"errors"
	"io"
	"os"
)

type Checked struct {
	Ok       bool                   `json:"ok"`
	Refusal  map[string]any         `json:"refusal"`
	Record   map[string]any         `json:"record"`
	Effects  []Effect               `json:"effects"`
}

type Effect struct {
	Capability string         `json:"capability"`
	Payload    map[string]any `json:"payload"`
	Reversible bool           `json:"reversible"`
}

type Decision map[string]any

type SDK struct{ checked Checked }

func (s SDK) Verify(token string) (Checked, error) {
	if !s.checked.Ok {
		return s.checked, errors.New("the record did not verify")
	}
	return s.checked, nil
}

func (s SDK) Issuance(c Checked, credential string) (map[string]any, bool) {
	for _, e := range c.Effects {
		if e.Capability != "credential.issue" {
			continue
		}
		if name, _ := e.Payload["credential"].(string); name == credential {
			claims, ok := e.Payload["claims"].(map[string]any)
			return claims, ok
		}
	}
	return nil, false
}

func (s SDK) Issue(credential string, claims map[string]any) Decision {
	return Decision{"kind": "issue", "credential": credential, "claims": claims}
}

func (s SDK) Accept(note string) Decision {
	return Decision{"kind": "accept", "note": note}
}

func (s SDK) Refuse(err error) Decision {
	refusal := s.checked.Refusal
	if refusal == nil {
		refusal = map[string]any{"kind": "refused", "why": err.Error()}
	}
	return Decision{"kind": "refuse", "refusal": refusal}
}

type payload struct {
	Token   string  `json:"token"`
	Checked Checked `json:"checked"`
}

func Main(handle func(string, SDK) Decision) {
	raw, err := io.ReadAll(os.Stdin)
	if err != nil {
		panic(err)
	}
	var p payload
	if err := json.Unmarshal(raw, &p); err != nil {
		panic(err)
	}
	json.NewEncoder(os.Stdout).Encode(handle(p.Token, SDK{checked: p.Checked}))
}
