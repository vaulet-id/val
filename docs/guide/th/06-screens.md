# หน้าจอ

คุณประกาศหน้าจอ แล้ว wallet เป็นคนวาด

```val
screen Wallet {
  data {
    receipts: credentials of PurchaseReceipt verified with ReceiptFromMerchant
      order by purchased_at desc
      limit 50
  }

  column {
    card(text: phrase("balance", points: state.member.points))
    section(text: "history")
    list(receipts) { r ->
      tile(text: phrase("receiptLine", merchant: r.claims.merchant, at: r.claims.purchased_at))
    }
    button(text: "scan", emphasis: primary, onTap: ScanToEarn)
  }
}
```

## หน้าจอเดียวที่เปิดแอป

ทำเครื่องหมาย `@main` ไว้ หน้าจอที่เหลือไปถึงด้วยการกด

```val
@main
screen Wallet { … }

screen Receipt(id: string) { … }
```

package ที่มีหน้าจอมากกว่าหนึ่งแล้วไม่มี `@main` เลย จะ build ไม่ผ่าน เพราะไม่อย่างนั้น
คนจะเห็นหน้าไหนก่อนขึ้นอยู่กับลำดับที่ไฟล์ถูกอ่าน

## หน้าจอหนึ่งแสดงคนละอย่างได้

```val
@main
screen Wallet {
  column {
    if (state.points > 0) {
      card(text: phrase("balance", points: state.points))
      button(text: "scan", emphasis: primary, onTap: ScanToEarn)
    } else {
      emptyState(text: "notAMember", detail: "joinAtTheCounter")
    }
  }
}
```

`else` ใส่หรือไม่ใส่ก็ได้ ทั้งสองกิ่งถูกตรวจไม่ว่ากิ่งไหนจะทำงาน และนับรวมเข้าไปใน
สิ่งที่ package ประกาศว่าตัวเองทำทั้งคู่ — capability ที่ใช้เฉพาะในกิ่งที่วันนี้ไม่มีใครเข้าถึง
ก็ยังเป็น capability ที่ผู้ใช้กดยินยอมไปแล้ว

wallet ไม่เคยเห็นเงื่อนไข มันถูกตัดสินตั้งแต่ตอน resolve หน้าจอ สิ่งที่ส่งไปให้วาดจึงเป็น
ต้นไม้ต้นเดียวที่เลือกเสร็จแล้ว

## ประกาศข้อมูลที่ต้องใช้ อย่าไปดึงเอง

wallet resolve บล็อก `data` ก่อนที่จะวาดอะไรทั้งสิ้น ไม่มีหน้าจอที่วาดค้างครึ่ง
เดียว และไม่มีกล่องขออนุญาตโผล่มาตอนที่ใครกำลังเลื่อนอ่าน

`verified with` แปลว่าใบเสร็จที่ไม่ผ่าน policy ปรากฏไม่ได้ ไม่ใช่ "ถูกกรองออก"
การกรองเป็นของ wallet ไม่ใช่บรรทัดในโค้ดคุณที่ใครสักคนลบทิ้งได้

`limit` จำเป็นสำหรับ list ที่คุณคำนวณต่อ มันจำกัดปริมาณงาน ซึ่งเป็นสิ่งที่ทำให้
ผลรวมของ list คอมไพล์ลงไปเป็นวงจรได้

## การกดระบุชื่อ action

`onTap` ระบุชื่อ action ที่คุณประกาศไว้ ไม่มี handler ชนิดอื่น ทุกอย่างที่หน้าจอ
เริ่มได้จึงผ่านหกเฟสด้วยความยินยอมชุดเดียวกันและ record ชุดเดียวกัน

## component เป็นของ wallet

```val
column { … }
section(text: "key")
card(text: phrase("key", name: value))
tile(text: phrase("key", name: value), onTap: Action)
list(binding) { item -> … }
button(text: "key", emphasis: primary, onTap: Action)
```

prop เป็นเชิงความหมาย: `text` `icon` `emphasis` `state` `onTap` ไม่มีสี
ไม่มีฟอนต์ ไม่มีขนาดพิกเซล การขอ component ที่ไม่มีในแคตตาล็อกจะถูกรายงาน
ไม่ใช่วาดของที่ใกล้เคียงออกมา

ถ้าคุณต้องการพิกเซล ให้ใช้ชั้น webview พร้อมเพดาน capability ที่ต่ำกว่า

## ข้อความไม่ได้อยู่ในโค้ดคุณ

`text: "balance"` เป็น key ที่ชี้เข้าไปใน bundle ที่ถูกเซ็นของคุณ:

```json
"balance": { "en": "You have {points} points", "th": "คุณมี {points} แต้ม" }
```

คุณให้ค่าใส่ช่อง แล้ว wallet จัดรูปแบบให้ เลขไทย พุทธศักราช และตำแหน่งสัญลักษณ์
สกุลเงินจึงถูกต้องครั้งเดียวสำหรับทุกแอป แทนที่จะผิดกันคนละแบบในสี่สิบแอป

## state ของการโต้ตอบไม่ใช่ state ของคุณ

แท็บไหนเปิดอยู่ list เลื่อนไปถึงไหน อะไรถูกพิมพ์ในช่องที่ยังไม่ได้ส่ง: เป็นของ
wallet ทั้งหมด คุณได้สิ่งที่ฟอร์มถืออยู่ ณ วินาทีที่มันถูกส่ง ผ่าน `input`

หมายเหตุ: `state` ของคุณถูกแฮช ถูกเซ็น และถูก replay ตำแหน่งการเลื่อนที่อยู่ในนั้น
จะเจือจางความหมายของคำว่า "พิสูจน์ได้" ทีละหนึ่งครั้งที่มีคนกด

## หน้าจออนุมานค่าได้ แต่ลงมือทำไม่ได้

```val
compute {
  const totalValue = holdings.fold(0) { sum, h -> sum + h.claims.market_value }
}
```

pure ใช้กติกาเดียวกับ action ไม่มี effect เก็บผลรวมไว้ที่นี่แทนที่จะเก็บใน
`state` — ค่าที่คำนวณได้จากสิ่งที่อยู่บนหน้าจออยู่แล้วไม่จำเป็นต้องถูกเก็บ

ต่อไป: [การเปิดเผยและการพิสูจน์](07-disclosing-and-proving.md)
