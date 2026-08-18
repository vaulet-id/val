# แอปแรกของคุณ

คุณจะสร้างบัตรสะสมแต้ม ผู้ใช้สแกนใบเสร็จ ได้หนึ่งแต้มต่อหนึ่งบาท แล้ว credential
สมาชิกของเขาถูกออกใหม่ที่ยอดแต้มใหม่

มันเล็กและใช้ครบทุกอย่าง: อ่าน credential ตรวจว่าใครออกให้ คำนวณ เปลี่ยน state
และขอให้ wallet ออกของใหม่

เปิด playground แล้วทำตามไปด้วย หรือสร้าง `loyalty.val` กับ `text.json`
ในไดเรกทอรีหนึ่ง

## 1. ประกาศว่าแอปคืออะไรและทำอะไรได้

```val
app "th.co.codefin.loyalty"
version 1

capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
}
```

นี่คือสองสิ่งเดียวที่แอปนี้จะได้รับอนุญาตให้ทำตลอดอายุของมัน และผู้ใช้เห็นทั้งคู่
ก่อนติดตั้ง

ข้อควรระวัง: การประกาศ capability ที่ไม่ได้ใช้ทำให้ build ล้มเหลว อย่าใส่
capability เผื่อไว้ก่อน

## 2. อธิบายข้อมูล

```val
enum Tier { bronze, silver, gold }

credential PurchaseReceipt {
  merchant:     string
  amount:       int        // สตางค์
  purchased_at: datetime
}

credential LoyaltyMember {
  member_id: string
  tier:      Tier
  points:    int
}

state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

`amount` เป็นสตางค์เพราะภาษานี้ไม่มี floating point เงินเป็นหน่วยย่อยเสมอ
ส่วนเปอร์เซ็นต์เป็น basis point

`state` คงอยู่ระหว่างแต่ละ action `member` เป็น optional — `LoyaltyMember?` —
คุณจึงอ่านทะลุเข้าไปข้างในไม่ได้จนกว่าจะบอกก่อนว่ามันมีอยู่

## 3. บอกว่าคุณเชื่อใคร

```val
trust ReceiptFromMerchant(receipt: PurchaseReceipt) {
  anchor: "th.co.codefin.merchants"
  require {
    receipt.signature.valid
    receipt.status.active
    receipt.holder.bound
    receipt.claims.amount > 0
  }
}
```

trust policy เป็นทางเดียวที่จะเข้าถึงสิ่งที่ credential บอก `anchor` ระบุรากที่
ห่วงโซ่จะถูกไล่ไปถึง ร้านค้าที่เข้าร่วมโครงการของคุณจึงไม่ได้แปลว่าต้องปล่อย
เวอร์ชันใหม่

สามการตรวจข้างบนมีอยู่ในทุก credential: ถูกเซ็นไหม ถูกเพิกถอนหรือยัง และถือโดย
คนที่อยู่ตรงหน้าจริงไหม

## 4. เขียน action

```val
action ScanToEarn {
  input {
    receipt: Credential<PurchaseReceipt>
  }

  require {
    state.member exists
  }

  verify {
    const checked = receipt with ReceiptFromMerchant
    checked.claims.purchased_at > context.time.now - duration(days: 30)
  }

  compute {
    if (checked.claims.amount < 2_000) { refuse "tooSmallToEarn" }

    const satangPerBaht = 100
    const earned = checked.claims.amount / satangPerBaht
    const total  = state.lifetimePoints + earned
    const tier   = tierFor(total)
  }

  update {
    lifetimePoints: total
    member.points:  state.member.points + earned
    member.tier:    tier
  }

  execute {
    credential.issue(LoyaltyMember {
      member_id: next.member.member_id,
      tier:      next.member.tier,
      points:    next.member.points,
    })
  }
}
```

หกเฟส เรียงตามลำดับนี้เสมอ จะละเฟสไหนก็ได้

| เฟส | มีไว้ทำอะไร |
| --- | --- |
| `require` | สิ่งที่ไม่ควรเป็นเท็จเลย ถ้าเป็นเท็จแปลว่าคุณมีบั๊ก และไม่มีใครถูกแสดงข้อความ |
| `verify` | ที่ที่ credential กลายเป็นของที่ใช้ได้ การล้มเหลวที่นี่เป็นเรื่องปกติ และผู้ใช้ได้รับแจ้ง |
| `compute` | การคำนวณแบบ pure ส่วน `refuse` ใช้ปฏิเสธด้วยเหตุผลของคุณเอง โดยระบุ key ใน `text.json` |
| `update` | state ถัดไป ในรูปตารางของสิ่งที่เปลี่ยน ใช้ `:` ไม่ใช่ `=` |
| `execute` | effect มันประกอบคำขอ wallet ไปถามผู้ใช้ แล้ว state ของคุณ commit ก็ต่อเมื่อเขาตกลง |

`receipt with ReceiptFromMerchant` ให้ผลเป็น `Verified<ReceiptFromMerchant>`
type นั้นเป็นทางเดียวที่จะเข้าถึง `.claims` การตรวจจึงลืมไม่ได้

## 5. ใส่ประโยคเข้าไป

ไม่มีประโยคที่ผู้ใช้อ่านอยู่ในโค้ดเลย ประโยคอยู่ใน `text.json`:

```json
{
  "locales": ["en", "th"],
  "keys": {
    "tooSmallToEarn": {
      "en": "Purchases under ฿20 do not earn points",
      "th": "ยอดต่ำกว่า 20 บาท ยังไม่ได้แต้ม"
    }
  }
}
```

key ที่โค้ดคุณเรียกแต่ bundle ไม่มี คือ build ที่ล้มเหลว และ key ที่ไม่มีคำแปล
สำหรับภาษาที่คุณปล่อยจริงก็เช่นกัน

## 6. ตรวจดู

```bash
valc loyalty.val
```

คุณจะได้ diagnostics ก่อน แล้วตามด้วยรายงาน capability — สิ่งที่ผู้ใช้จะถูกแสดง
อนุมานจากโค้ดของคุณ:

```
th.co.codefin.loyalty v1
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
discloses      —
issues         LoyaltyMember
writes state   lifetimePoints, member.points, member.tier
irreversible   none
```

อ่านมันอย่างที่ผู้ใช้ของคุณจะอ่าน ถ้ามันบอกอะไรที่คุณไม่ได้ตั้งใจ แปลว่าโค้ดของคุณ
บอกอย่างนั้น — รายงานนี้แก้ไม่ได้ และ wallet ก็คำนวณใหม่เองอยู่ดี

ต่อไป: [capability กับความยินยอม](03-capabilities.md)
