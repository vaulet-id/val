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

คุณพูดให้น้อยกว่าความจริงไม่ได้ wallet คำนวณรายงานใหม่จาก source ของคุณ และปฏิเสธ
package ถ้ามันไม่ตรงกับที่คุณส่งมา

หมายเหตุ: นี่คือเหตุผลที่ source เดินทางไปใน package ด้วย แฮชของผลลัพธ์ที่คอมไพล์
แล้วพิสูจน์ว่ามันคือผลลัพธ์ที่ใครบางคนเซ็น แต่ไม่เคยพิสูจน์ว่ามันคือโปรแกรมที่
ใครบางคนอ่าน

ต่อไป: [credential กับความเชื่อถือ](04-credentials-and-trust.md)
