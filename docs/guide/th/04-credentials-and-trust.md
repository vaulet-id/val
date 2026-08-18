# Credential กับความเชื่อถือ

credential คือคำพูดของใครคนหนึ่งเกี่ยวกับอีกคนหนึ่ง ที่ถูกเซ็นไว้ หน้าที่ของแอป
คุณคือบอกว่า **จะยอมรับคำพูดของใคร** และภาษานี้จะไม่ยอมให้คุณอ่านมันจนกว่าคุณจะบอก

## สี่ด้าน

ทุก credential มีรูปร่างเดียวกัน ไม่ว่ามันจะบรรจุอะไร:

```val
receipt.claims        สิ่งที่ผู้ออกพูด — ฟิลด์ของ credential คุณ
receipt.signature     .valid
receipt.status        .active
receipt.holder        .bound — ใช่คนที่อยู่ตรงหน้าเราไหม
```

สามอันหลังอ่านได้ใน `trust` และ `verify` เท่านั้น ที่อื่นอ่านไม่ได้ แอปที่ตัดสิน
เองว่าลายเซ็นดีพอหรือยัง คือสิ่งที่ trust policy มีไว้เพื่อหยุดพอดี

## policy คือชุดเงื่อนไขที่มีชื่อ

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

ตัวประธานถูกผูกด้วยชื่อ — `holding.signature.valid` ไม่ใช่ `signature.valid`
ลอยๆ ซึ่งไม่กำกวมจนกระทั่งวินาทีที่มี credential ตัวที่สองอยู่ใน scope

บรรทัดสุดท้ายนั่นคือบรรทัดที่ควรลอกไปใช้ ราคาประเมินจากอาทิตย์ที่แล้วถูกเซ็น
ไม่ถูกเพิกถอน ผูกกับผู้ถือถูกต้อง — และผิด **ความสดใหม่เป็นคำถามเรื่องความเชื่อถือ**
มันจึงอยู่ใน policy แล้วข้อมูลเก่าก็ไปไม่ถึงหน้าจอคุณเลย แทนที่จะไปถึงพร้อมคำเตือน
ที่ใครสักคนต้องจำว่าต้องใส่

## anchor ไม่ใช่ผู้ออก

`anchor:` ระบุรากที่ห่วงโซ่จะถูกไล่ไปถึง การปักหมุดผู้ออกรายใดรายหนึ่งทำได้ และ
มักเป็นความผิดพลาด: มันเอา allowlist ของคุณไปไว้ในที่ที่ผู้ใช้มองไม่เห็น และการ
เพิ่มร้านค้าหนึ่งร้านก็กลายเป็นแอปเวอร์ชันใหม่

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
และไม่มีทางเข้าถึง `claims` โดยไม่ผ่านมัน การตรวจจึงลืมไม่ได้ — ลืมแล้วคอมไพล์ไม่ผ่าน

**type ระบุชื่อ policy ไม่ใช่ชื่อ credential** `Verified<SignatureOnly>` กับ
`Verified<ReceiptFromMerchant>` เป็นคนละ type และฟังก์ชันที่ต้องการอันหลังจะไม่รับ
อันแรก ลายเซ็นที่ถูกต้องไม่ได้บอกอะไรเลยว่า credential ถูกเพิกถอนหรือยังหรือใคร
เป็นคนออก และ type system ที่เรียกทั้งคู่ว่า "ตรวจแล้ว" ก็เท่ากับบอกคุณว่ามัน
เป็นข้อเท็จจริงเดียวกัน

ถ้า policy หนึ่งครอบคลุมอีกอันจริงๆ ก็บอกออกมา:

```val
trust StrictReceipt(r: PurchaseReceipt) refines ReceiptFromMerchant { … }
```

ตรวจแบบการบรรจุ — policy ของคุณต้องเรียกร้องทุกอย่างที่อีกอันเรียกร้อง ในระดับ
ตัวอักษร ไม่ใช่ตรวจแบบการอนุมาน: ตัวตรวจที่ตัดสินว่า predicate หนึ่งอนุมานไปถึง
อีกอันได้ คือตัวตรวจที่ผิดแบบเงียบๆ

## ที่มาของค่าเดินทางไปกับตัวค่า

ตัวเลขที่คำนวณจาก credential ที่ตรวจแล้วจำที่มานั้นไว้ ตัวเลขที่คำนวณจาก state
ของคุณเอง หรือจากคำตอบของ API ก็จำที่มาของมันไว้เหมือนกัน — และทั้งสองไม่ใช่
ข้อเท็จจริงเดียวกัน

คุณจะเจอเรื่องนี้ตอนออก credential:

```val
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

`from` บอกว่า claim นี้คำนวณได้จากข้อมูลที่ตรวจภายใต้ policy นั้นเท่านั้น ผสม
อย่างอื่นเข้าไปแล้วคอมไพล์ไม่ผ่าน สิ่งที่ได้กลับมาคือคนที่รับ credential นี้ต่อ
ไม่ต้องเชื่อลายเซ็นของคุณเรื่องวิธีที่ตัวเลขนั้นถูกคำนวณ

ต่อไป: [action](05-actions.md)
