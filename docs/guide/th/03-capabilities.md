# Capability กับความยินยอม

ทุกอย่างที่แอปของคุณทำได้อยู่ในบล็อกเดียว และผู้ใช้ตกลงกับบล็อกนั้นก่อนที่มันจะรัน

```val
capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
  disclosure.present
  api.query(audience: "broker.co.th", presenting: BrokerageAccount)
}
```

## กติกา

**มันระบุ type ไม่ใช่ string** `credential.read(PurchaseReceipt)` ถูกคอมไพเลอร์
ตรวจ การประกาศ type หนึ่งแล้วไปอ่านอีก type หนึ่งคอมไพล์ไม่ผ่าน

**หนึ่งบล็อกต่อหนึ่ง package** package มีได้หลายไฟล์ แต่ capability ถูกประกาศ
ครั้งเดียว คำถามว่า "ไฟล์ไหนบอกว่าแอปนี้ทำอะไรได้" มีคำตอบเดียว

**ประกาศแล้วไม่ใช้ = ล้มเหลว** การประกาศสิ่งที่ไม่เคยใช้ทำให้ build ล้มเหลว

หมายเหตุ: นี่คือกฎที่คนคัดค้านมากที่สุด แอปที่ขอมากกว่าที่ต้องการฝึกให้ผู้ใช้
เลิกอ่าน แล้วแอปตัวถัดไปก็คือตัวที่ควรจับตาจริงๆ

## สิ่งที่ผู้ใช้เห็น

ไม่ใช่บล็อกนี้ เขาเห็น **รายงาน capability** ซึ่งคอมไพเลอร์อนุมานจากโค้ดของคุณ:

```
reads          PurchaseReceipt.amount, PurchaseReceipt.purchased_at
               under ReceiptFromMerchant
discloses      NationalId.country
proves         birthdate <= now - 20 years
issues         LoyaltyMember
talks to       broker.co.th
writes state   member.points
irreversible   one disclosure
```

มันไล่รายการ claim ที่คุณแตะ ไม่ใช่แค่ตัว credential ของอะไรไปไหนบ้าง และมีอะไร
เกิดขึ้นที่เอาคืนไม่ได้หรือเปล่า

คุณพูดให้น้อยกว่าความจริงไม่ได้ wallet วัดรายงานจาก import section ของ module เอง
— สิ่งที่ module เรียกได้คือสิ่งที่มัน import — แล้วปฏิเสธ package ถ้ามันไม่ตรงกับ
ที่คุณส่งมา

## แอปของคุณเปิดให้ใคร

แอปส่วนใหญ่เปิดให้ทุกคนที่ถือเครื่องอยู่ ถ้าของคุณไม่ใช่ ก็บอกไว้:

```val
capabilities { credential.check(EmployeeBadge) }

admits {
  EmployeeBadge with EmployedByAcme else "notStaff"
}
```

ถ้าไม่มี credential ที่ผ่าน policy แอปจะไม่วาดหน้าจอแรกและไม่รัน action ผู้ใช้เห็น
`notStaff` ซึ่งเป็น key ใน text bundle ของคุณ คำพูดจึงเป็นของคุณ ผ่านการทบทวนและ
แปลเหมือนทุกประโยคที่คุณส่ง

**wallet เป็นคนตอบที่ประตู โปรแกรมของคุณไม่เคยถาม** ในภาษานี้เขียนว่า "คนนี้ถืออันนี้
อยู่ไหม" ไม่ได้ และนั่นคือประเด็น: โปรแกรมที่ถามได้คือโปรแกรมที่ต้องถือ credential
ไว้เพื่อจะรู้ว่ามันไม่มี นั่นคือการอ่าน และประตูมีไว้แทนมัน บรรทัดข้างบนจึงเป็น
`credential.check` ไม่ใช่ `credential.read` — แอปของคุณถูกบอกว่ามันเปิดแล้ว ไม่เคย
ถูกบอกว่าอะไรเปิดมัน และ sheet ก็พูดแบบนั้นพอดี

ทั้งสองครึ่งเป็นสิ่งบังคับ ไม่มี policy คุณก็รับอะไรก็ได้ที่รูปร่างเหมือนบัตร รวมถึง
บัตรที่ใครทำขึ้นเอง ไม่มีประโยคประตูก็ปิดเงียบๆ แล้วผู้ใช้เหลือแค่รายงานความผิดพลาด
แทนคำแนะนำ

ต่อไป: [credential กับความเชื่อถือ](04-credentials-and-trust.md)
