"""The SDK a Python handler is given.

Verification already happened, in Rust, before this process started. There is
one verifier and every language talks to it the same way: the runner hands the
result in on stdin and this module shapes it.
"""

import json
import sys


class Checked:
    def __init__(self, data):
        self.ok = data.get("ok", False)
        self.refusal = data.get("refusal")
        self.record = data.get("record", {})
        self.effects = data.get("effects", [])


class Sdk:
    def __init__(self, checked):
        self._checked = checked

    def verify(self, token):
        return self._checked

    def issuance(self, checked, credential):
        for e in checked.effects:
            if e.get("capability") != "credential.issue":
                continue
            payload = e.get("payload") or {}
            if payload.get("credential") == credential:
                return payload.get("claims")
        return None

    def issue(self, credential, claims):
        return {"kind": "issue", "credential": credential, "claims": claims}

    def accept(self, note):
        return {"kind": "accept", "note": note}

    def refuse(self, refusal):
        return {"kind": "refuse", "refusal": refusal}


def main(handle):
    payload = json.load(sys.stdin)
    sdk = Sdk(Checked(payload["checked"]))
    decision = handle(payload["token"], sdk)
    json.dump(decision, sys.stdout)
