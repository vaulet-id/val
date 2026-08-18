# Credential กับความเชื่อถือ

credential คือคำพูดที่ถูกเซ็นของใครคนหนึ่งเกี่ยวกับอีกคนหนึ่ง แอปของคุณบอกว่าจะ
ยอมรับคำพูดของใคร และภาษาจะไม่ยอมให้คุณอ่านมันจนกว่าคุณจะบอก

## สี่ด้าน

```val
receipt.claims        // สิ่งที่ผู้ออกพูด — ฟิลด์ที่คุณประกาศ
receipt.signature     // .valid
receipt.status        // .active
receipt.holder        // .bound — ใช่คนที่อยู่ตรงหน้าเราไหม
```

สามอันหลังอ่านได้ใน `trust` และ `verify` เท่านั้น

## เขียน policy

```val
trust FromLicensedBroker(holding: Holding) {
  anchor: "th.go.sec.licensed-brokers"
  require {
    holding.signature.valid
    holding.status.active
    holding.holder.bound
    holding.claims.valued_at > context.time.now - duration(hours: 24)
  }
}
```

ผูกตัวประธานด้วยชื่อ — `holding.signature.valid` ไม่ใช่ `signature.valid`
เปลือกๆ ซึ่งจะเลิกไม่กำกวมทันทีที่มี credential ตัวที่สองอยู่ใน scope

**ใส่เรื่องความสดใหม่ไว้ใน policy** ราคาประเมินจากอาทิตย์ที่แล้วถูกเซ็น
ไม่ถูกเพิกถอน ผูกกับผู้ถือถูกต้อง — และผิด เมื่ออยู่ใน policy ข้อมูลเก่าก็ไปไม่ถึง
หน้าจอของคุณเลย

**ใช้ anchor ไม่ใช่ผู้ออก** การปักหมุดผู้ออกรายใดรายหนึ่งเอา allowlist ของคุณไปไว้
ในที่ที่ผู้ใช้มองไม่เห็น และการเพิ่มร้านค้าหนึ่งร้านก็กลายเป็นเวอร์ชันใหม่

## การตรวจสอบเป็น type

```val
verify {
  const checked = receipt with ReceiptFromMerchant
}

compute {
  const earned = checked.claims.amount / 100
}
```

`checked` เป็น `Verified<ReceiptFromMerchant>` ไม่มี cast ที่สร้างมันขึ้นมาได้
การลืมตรวจจึงคอมไพล์ไม่ผ่าน

**type ระบุชื่อ policy ไม่ใช่ชื่อ credential** `Verified<SignatureOnly>` กับ
`Verified<ReceiptFromMerchant>` เป็นคนละ type ลายเซ็นที่ถูกต้องไม่ได้บอกอะไรเลย
เรื่องการเพิกถอนหรือเรื่องว่าใครเป็นคนออก

ถ้า policy หนึ่งครอบคลุมอีกอันจริงๆ ให้ประกาศออกมา:

```val
trust StrictReceipt(r: PurchaseReceipt) refines ReceiptFromMerchant { … }
```

ตรวจแบบการบรรจุ: policy ของคุณต้องเรียกร้องทุกอย่างที่อีกอันเรียกร้อง ในระดับ
ตัวอักษร

## ที่มาของค่า

ตัวเลขที่คำนวณจาก credential ที่ตรวจแล้วจำที่มานั้นไว้ ตัวเลขจาก state ของคุณเอง
หรือจากคำตอบของ API ก็จำที่มาของมันไว้เหมือนกัน — และทั้งสองไม่ใช่ข้อเท็จจริง
เดียวกัน

คุณจะเจอเรื่องนี้ตอนออก credential:

```val
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

`from` บอกว่า claim นี้คำนวณได้จากข้อมูลที่ตรวจภายใต้ policy นั้นเท่านั้น
ผสมอย่างอื่นเข้าไปแล้วคอมไพล์ไม่ผ่าน คนที่รับ credential ไปจึงตรวจได้เองว่า
ตัวเลขนั้นถูกคำนวณมาอย่างไร

ต่อไป: [action](05-actions.md)
