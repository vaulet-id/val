# แอปแรกของคุณ

บัตรสะสมแต้ม ผู้ใช้สแกนใบเสร็จ ได้หนึ่งแต้มต่อหนึ่งบาท แล้ว credential
สมาชิกของเขาถูกออกใหม่ที่ยอดแต้มใหม่

เป็นแอปเล็กๆ ที่ใช้ครบทุกอย่าง: อ่าน credential ตรวจว่าใครออกให้ คำนวณ
เปลี่ยน state และขอให้ host ออกของใหม่ สร้างอันนี้ได้แล้ว ที่เหลือในคู่มือ
คือรายละเอียด

## บอกว่าคุณเป็นใคร และทำอะไรได้บ้าง

```val
app "th.co.codefin.loyalty"
version 1

capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
}
```

สองสิ่งที่ผู้ใช้จะเห็นก่อนติดตั้ง และเป็นสองสิ่งเดียวที่แอปนี้จะได้รับอนุญาต
ให้ทำตลอดอายุของมัน

ประกาศ capability ที่ไม่ได้ใช้แล้ว build จะล้มเหลว นั่นไม่ใช่เรื่องความเรียบร้อย:
ความยินยอมที่ขอไปเพื่อสิ่งที่ไม่ได้ใช้คือความยินยอมที่จ่ายไปเปล่าๆ และเป็นวิธี
ที่สอนให้คนกดตกลงโดยไม่อ่าน

## อธิบายข้อมูล

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

`amount` เป็นสตางค์เพราะ**ภาษานี้ไม่มี floating point** เงินในภาษานี้เป็นหน่วยย่อย
เสมอ — และอะไรก็ตามที่อยากมีจุดทศนิยมก็เช่นกัน จำนวนหุ้นคือไมโครหุ้น
เปอร์เซ็นต์คือ basis point

`state` เป็นของคุณ และคงอยู่ข้ามแต่ละ action `member` เป็น optional เขียนว่า
`LoyaltyMember?` ซึ่งแปลว่าคุณอ่านทะลุเข้าไปข้างในไม่ได้จนกว่าจะบอกก่อนว่ามันมีอยู่

## บอกว่าคุณเชื่อใคร

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

trust policy เป็นทางเดียวที่จะเข้าถึงสิ่งที่ credential บอก มันระบุ **anchor**
— รากที่ห่วงโซ่จะถูกไล่ไปถึง — แทนที่จะระบุผู้ออกรายใดรายหนึ่ง ร้านค้าที่เข้าร่วม
โครงการของคุณจึงไม่ได้แปลว่าต้องปล่อยแอปเวอร์ชันใหม่

สามด้านข้างบนมีอยู่ในทุก credential: ถูกเซ็นไหม ถูกเพิกถอนหรือยัง และถือโดยคนที่
อยู่ตรงหน้าจริงไหม แอปที่ตัดสินสามข้อนี้เองคือสิ่งที่ trust policy มีไว้เพื่อป้องกัน

## เขียน action

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

หกเฟส เรียงตามลำดับนั้น และคุณจะละเฟสไหนก็ได้

**`require`** ไว้สำหรับสิ่งที่ไม่ควรเป็นเท็จเลย ถ้ามันเป็นเท็จ แปลว่าแอปคุณมีบั๊ก:
action หยุด และไม่มีใครถูกแสดงข้อความอะไร

**`verify`** คือที่ที่ credential กลายเป็นของที่ใช้ได้ `receipt with
ReceiptFromMerchant` ให้ผลเป็น `Verified<ReceiptFromMerchant>` — และ type นั้น
ซึ่งระบุชื่อ policy ไว้ เป็นทางเดียวที่จะเข้าถึง `claims` การล้มเหลวที่นี่เป็น
เรื่องปกติ: ใบเสร็จปลอมหรือหมดอายุ แล้วผู้ใช้ก็ได้รับแจ้ง

**`compute`** เป็น pure ไม่มี effect และ `refuse` คือวิธีปฏิเสธด้วยเหตุผลของ
คุณเอง โดยระบุ key ใน text bundle เพราะประโยคที่คนอ่านต้องเป็นประโยคที่ถูกเซ็นมาแล้ว

**`update`** อธิบาย state ถัดไปในรูปตารางของสิ่งที่เปลี่ยน ไม่ใช่การกำหนดค่า:
ใช้ทวิภาค เพราะบรรทัดนั้นบอกว่า state ถัดไปคืออะไร ในขณะที่ state ก่อนหน้า
ยังอ่านได้อยู่ทางขวาของมัน

**`execute`** เป็นที่เดียวที่ effect ปรากฏ — และมันไม่ได้ลงมือทำ มันประกอบคำขอ
host ไปถามผู้ใช้ แล้ว state ของคุณจะ commit ก็ต่อเมื่อผู้ใช้ตอบตกลง

## ใส่ประโยคเข้าไป

ไม่มีอะไรในโค้ดข้างบนที่เป็นประโยคให้คนอ่าน ประโยคเหล่านั้นอยู่ใน `text.json`
หนึ่ง entry ต่อหนึ่ง key หนึ่งบรรทัดต่อหนึ่งภาษา:

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

key ที่ไม่มีคำแปลสำหรับภาษาที่คุณปล่อยจริง คือ **build ที่ล้มเหลว** ไม่ใช่
รายงานบั๊ก และ key ที่โค้ดคุณเรียกแต่ bundle ไม่มี ก็เช่นกัน

## ตรวจดู

```bash
valc loyalty.val        # อ่าน text.json ที่วางอยู่ข้างๆ
```

diagnostics มาก่อน แล้วตามด้วยรายงาน capability — ซึ่งคือสิ่งที่ผู้ใช้จะถูกแสดง
อนุมานจากโค้ดของคุณ ไม่ใช่คุณเขียนเอง:

```
th.co.codefin.loyalty v1
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
discloses      —
issues         LoyaltyMember
writes state   lifetimePoints, member.points, member.tier
irreversible   none
```

อ่านมันอย่างที่ผู้ใช้ของคุณจะอ่าน ถ้ามันบอกอะไรที่คุณไม่ได้ตั้งใจ แปลว่าโค้ดบอก
อย่างนั้น — รายงานนี้แก้ไม่ได้ และ host ก็คำนวณใหม่เองอยู่ดี

ต่อไป: [capability กับความยินยอม](03-capabilities.md)
