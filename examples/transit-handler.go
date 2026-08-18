// Runs on the operator's server, not on the phone.
//
// A ride is worth a receipt only if the record that arrived is one this operator
// published, committed, and shows a ride being issued. Everything else is a
// refusal with a reason, which is what a gate needs to display.

package handler

import (
	"fmt"

	"runner/val"
)

// A fare cap: past this many rides in one pass, the operator stops issuing
// receipts and the rider is told why rather than charged again.
const monthlyCap = 40

func Handle(token string, v val.SDK) val.Decision {
	// Signature, code hash, outcome and rollback, all checked before this line.
	checked, err := v.Verify(token)
	if err != nil {
		return v.Refuse(err)
	}

	claims, ok := v.Issuance(checked, "RideReceipt")
	if !ok {
		return v.Accept("no receipt requested")
	}

	// Read the count from the record, never from anything the caller asked to
	// have signed. Signing the request instead would make every check above
	// decorative.
	count, _ := claims["ride_count"].(float64)
	if int(count) > monthlyCap {
		return v.Refuse(fmt.Errorf("this pass has reached %d rides", monthlyCap))
	}

	return v.Issue("RideReceipt", claims)
}
