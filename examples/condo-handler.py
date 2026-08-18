# Runs on the juristic person's server, not on the phone.
#
# The meeting's own arithmetic is not something to take on trust from the app
# that computed it. This checks the cap the law sets before it records anything,
# and refuses a vote that claims more weight than one owner may have.

# A third of the building, in basis points. Section 45 of the Condominium Act:
# one owner's votes count for no more than this, however many units they hold.
STATUTORY_CAP_BP = 3_333


def handle(token, val):
    # Signature, code hash, outcome and rollback, all checked before this line.
    checked = val.verify(token)
    if not checked.ok:
        return val.refuse(checked.refusal)

    claims = val.issuance(checked, "VoteCounted")
    if claims is None:
        return val.accept("nothing to record")

    # Read the weight out of the record, never out of a request. Recording what
    # the caller asked for would make every check above decorative.
    weight = claims["weight_bp"]
    if weight > STATUTORY_CAP_BP:
        return val.refuse({
            "kind": "policy",
            "why": f"{weight} bp exceeds the {STATUTORY_CAP_BP} bp one owner may cast",
        })

    return val.issue("VoteCounted", claims)
