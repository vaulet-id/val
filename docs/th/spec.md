# VAL

ภาษาสำหรับเขียน Micro App — แอปเล็กๆ ที่รันอยู่ใน wallet ของผู้ใช้ ข้างพาสปอร์ต
และ credential ธนาคารของเขา

```val
app "th.co.codefin.loyalty"
version 1

capabilities {
  credential.read(PurchaseReceipt)
  credential.issue(LoyaltyMember)
}
```

Micro App อ่าน credential ที่ผู้ใช้ถืออยู่แล้ว คำนวณ แล้วขอให้ wallet ลงมือทำ —
ออก credential รับชำระเงิน หรือพิสูจน์ข้อเท็จจริง

สี่อย่างที่คุณได้โดยไม่ต้องสร้างเอง:

- **ไม่ต้องมี login** ผู้ใช้ถูกระบุตัวตนแล้วด้วย credential ที่รัฐ ธนาคาร หรือ
  นายจ้างออกให้
- **ไม่ต้องเก็บข้อมูลผู้ใช้** ข้อมูลอยู่ใน wallet ของเขา คุณอ่าน claim ภายใต้
  policy ที่คุณระบุชื่อไว้
- **พิสูจน์ได้โดยไม่ต้องเปิดเผย** ถามว่าอายุเกินยี่สิบไหมได้โดยไม่ต้องรู้วันเกิด
- **record ที่ถูกเซ็นของทุกการรัน** ส่งให้ลูกค้าหรือผู้ตรวจสอบได้

## อะไรที่ต่างจากภาษาที่คุณรู้จัก

| | |
| --- | --- |
| ประกาศก่อนใช้ | ทุกอย่างที่แอปทำได้อยู่ในบล็อก `capabilities` ใช้สิ่งที่ไม่ได้ประกาศ = build ล้มเหลว ประกาศแล้วไม่ใช้ = build ล้มเหลวเช่นกัน |
| ไม่มี floating point | เงินเป็นหน่วยย่อย เช่นสตางค์ เปอร์เซ็นต์เป็น basis point |
| ไม่มี loop ไม่มี recursion | ใช้ `map` `filter` `fold` `any` `all` `count` `first` กับ list ทุกโปรแกรมหยุด |
| ประกอบ string ไม่ได้ | ไม่มี `+` ไม่มี interpolation ทุกประโยคที่ผู้ใช้อ่านมาจาก `text.json` ผ่าน key |
| วาดหน้าจอเองไม่ได้ | คุณประกาศ `card` `row` `button` แล้ว wallet เป็นคนวาด |
| ไม่มีการกำหนดค่า | มีแต่ `const` record สร้างต่อยอดด้วย spread ส่วน state เปลี่ยนผ่านบล็อก `update` |
| error เป็นผลลัพธ์ | ไม่มี `Result` ไม่มี exception ไม่มี early return action จะ commit หรือไม่ commit |

**เพิ่งเริ่ม?** ไปที่ [แอปแรกของคุณ](../guide/th/02-your-first-application.md)
ซึ่งสร้างบัตรสะสมแต้มที่ใช้งานได้จริง เอกสารนี้คือที่ที่คุณกลับมาเปิดดูทีหลัง

---

## โครงสร้างของโปรแกรม

โปรแกรมหนึ่งคือ **package**: ไฟล์ `.val` หนึ่งไฟล์ขึ้นไป บวก `text.json`
ไฟล์เหล่านั้นใช้ scope ร่วมกัน — ไม่มี import และไม่มี namespace ต่อไฟล์
หน้าจอในไฟล์หนึ่งจึงเรียก action ที่ประกาศในอีกไฟล์ได้

```val
app "th.co.codefin.loyalty"     // reverse-DNS ในเครื่องหมายคำพูด
version 1

capabilities { … }              // สิ่งที่แอปนี้ทำได้ ประกาศครั้งเดียวต่อ package
enum · credential · type        // การประกาศข้อมูล
state { … }                     // สิ่งที่คงอยู่ระหว่าง action
trust … { … }                   // policy การตรวจสอบที่มีชื่อ
function … { … }                // helper แบบ pure
action … { … }                  // สิ่งเดียวที่รันได้
screen … { … }                  // สิ่งที่ผู้ใช้เห็น
```

### ไวยากรณ์

- **การขึ้นบรรทัดใหม่จบคำสั่ง** ไม่มีอัฒภาค คำสั่งเดินต่อได้ระหว่างที่วงเล็บ
  ยังไม่ปิด นิพจน์ยาวๆ จึงห่อด้วย `( … )`
- **เปลือกคั่นด้วยการขึ้นบรรทัดใหม่ นิพจน์คั่นด้วยจุลภาค**
  `member.tier: tier` อยู่บรรทัดของมันเอง ส่วน `{ ...member, tier: tier }` ใช้จุลภาค
- **`if` และ `switch` ใส่วงเล็บให้เงื่อนไข** เหมือน TypeScript และ Dart
- **ตัวเลขเป็นฐานสิบ** ใส่ `_` ระหว่างหลักได้: `100_000` ไม่มีฐานสิบหก
  ไม่มีฐานสอง ไม่มีเลขยกกำลัง และ `12.50` เป็น error
- **ตัวระบุเป็น ASCII และเป็น camelCase** สำหรับชื่อที่คุณตั้งเอง ส่วนชื่อ claim
  จากผู้ออกใช้การสะกดของเขา: `purchased_at` `document_number`
- **อาร์กิวเมนต์ระบุชื่อเมื่อมีตั้งแต่สองตัว**: `payment.request(to: merchant,
  amount: 12000)` ตัวเดียวเป็นแบบตำแหน่งได้
- **คีย์เวิร์ดถูกจอง** และจุดคือการเข้าถึงฟิลด์เสมอ

---

## Type

| type | หมายเหตุ |
| --- | --- |
| `int` | 64 บิตมีเครื่องหมาย trap เมื่อล้นและเมื่อหารด้วยศูนย์ |
| `string` | เทียบและส่งต่อได้ ประกอบไม่ได้ |
| `bool` | `true`, `false` |
| `date`, `datetime` | เทียบกันแบบจำนวนเต็ม บวก `duration` แล้วได้ type เดิม |
| `bytes` | |
| `List<T>` | ไม่มี index ใช้ผ่าน combinator |
| `T?` | optional ทำให้แคบลงด้วย `exists` ใน `require` |
| `Credential<T>` | ถืออยู่แต่ยังไม่ตรวจ claim ของมันเอื้อมไม่ถึง |
| `Verified<P>` | สิ่งที่ `verify` ผลิต `P` คือ **policy** ไม่ใช่ credential |
| `Proof<bool>` | สิ่งที่ `prove` ผลิต |

### ประกาศข้อมูล

```val
enum Tier { bronze, silver, gold }

credential PurchaseReceipt {      // คนอื่นเป็นคนเซ็น
  merchant:     string
  amount:       int               // สตางค์
  purchased_at: datetime
}

type Quote {                      // record ธรรมดา ไม่มีใครเซ็น
  symbol: string
  price:  int
}

state {
  member:         LoyaltyMember?
  lifetimePoints: int default 0
}
```

ฟิลด์ใน state ใช้ `default` ไม่ใช่ `=` ภาษานี้ไม่มีการกำหนดค่าที่ไหนเลย

### ทำงานกับค่า

```val
const bumped = { ...member, points: member.points + earned }   // สร้างต่อยอด ไม่แก้ของเดิม
const fee    = amount > 100_000 ? 0 : 20                       // นิพจน์เงื่อนไข

const discount = switch (tier) {
  Tier.bronze => 0,
  Tier.silver => 5,
  Tier.gold   => 10,
}
```

`switch` เหนือ enum **มี `default` ไม่ได้** การเพิ่ม `Tier.platinum` จึงทำให้
ทุกโปรแกรมที่ตัดสินอะไรต่อระดับชั้นพัง ส่วน `switch` เหนือ `int` หรือ `string`
ต้องมี `default` และกิ่งที่ไปไม่ถึงเป็น error

---

## Credential กับความเชื่อถือ

ทุก credential มีสี่ด้านเหมือนกัน:

```val
receipt.claims        // สิ่งที่ผู้ออกพูด — ฟิลด์ที่คุณประกาศ
receipt.signature     // .valid
receipt.status        // .active
receipt.holder        // .bound — ใช่คนที่อยู่ตรงหน้าเราไหม
```

สามอันหลังอ่านได้ใน `trust` และ `verify` เท่านั้น

### เขียน policy

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

`anchor` ระบุรากที่ห่วงโซ่ใบรับรองจะถูกไล่ไปถึง การเพิ่มร้านค้าเข้าโครงการของคุณ
จึงไม่ได้แปลว่าต้องปล่อยแอปเวอร์ชันใหม่

ใส่เรื่องความสดใหม่ไว้ใน policy ด้วย ราคาประเมินจากอาทิตย์ที่แล้วถูกเซ็น
ไม่ถูกเพิกถอน ผูกกับผู้ถือถูกต้อง — และผิด:

```val
holding.claims.valued_at > context.time.now - duration(hours: 24)
```

### ใช้งาน

```val
verify {
  const checked = receipt with ReceiptFromMerchant
}

compute {
  const earned = checked.claims.amount / 100
}
```

`checked` มี type เป็น `Verified<ReceiptFromMerchant>` ไม่มี cast ที่สร้างมันได้
และไม่มีทางเข้าถึง `.claims` โดยไม่ผ่านมัน

**type ระบุชื่อ policy** `Verified<SignatureOnly>` กับ
`Verified<ReceiptFromMerchant>` เป็นคนละ type และฟังก์ชันที่ต้องการอันหลังจะไม่รับ
อันแรก ถ้า policy หนึ่งครอบคลุมอีกอันจริงๆ ให้ประกาศออกมา:

```val
trust StrictReceipt(r: PurchaseReceipt) refines ReceiptFromMerchant { … }
```

### ที่มาของค่า

ทุกค่าจำไว้ว่ามันสืบทอดมาจาก policy ไหนบ้าง และคอมไพเลอร์แพร่มันต่อให้เอง
คุณเขียนมันเองที่เดียว คือบน claim ที่คุณออก:

```val
credential.issue(LoyaltyMember {
  points: next.member.points from { ReceiptFromMerchant }
})
```

`from` เป็นข้อเรียกร้อง: claim นี้คำนวณได้จากข้อมูลที่ตรวจภายใต้ policy นั้น
เท่านั้น ผสมอย่างอื่นเข้าไปแล้วคอมไพล์ไม่ผ่าน คนที่รับ credential ไปจึงตรวจได้เอง
ว่าตัวเลขนั้นถูกคำนวณมาอย่างไร แทนที่จะต้องเชื่อลายเซ็นของคุณ

---

## Action

action เป็นสิ่งเดียวที่รันได้:

```
(state ก่อนหน้า, input, บริบทตอนรัน, โค้ด) → (state ใหม่, output, effect)
```

```
input → require → verify → compute → update → execute
```

จะละเฟสไหนก็ได้ แต่ห้ามสลับลำดับ

| เฟส | ใส่อะไรลงไป |
| --- | --- |
| `input` | สิ่งที่ action ได้รับมา |
| `require` | เงื่อนไขก่อนหน้า และการทำให้ `T?` แคบลงด้วย `exists` |
| `verify` | trust policy |
| `compute` | การคำนวณแบบ pure และ `refuse` |
| `update` | state ถัดไป ในรูป patch |
| `execute` | effect — เฟสเดียวที่ effect ปรากฏได้ |

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

    const earned = checked.claims.amount / 100
    const total  = state.lifetimePoints + earned
  }

  update {
    lifetimePoints: total
    member.points:  state.member.points + earned
  }

  execute {
    credential.issue(LoyaltyMember {
      member_id: next.member.member_id,
      points:    next.member.points,
    })
  }
}
```

ชื่อจาก `input` อยู่ใน scope ของทุกเฟสหลังจากนั้นแบบเปลือย ส่วนรากที่มีคำนำหน้า
คือ `state.` `context.` และ `next.` ภายใน `execute`

### สี่ทางที่ action ไม่ commit

เลือกให้ถูก — นี่คือความผิดพลาดที่พบบ่อยที่สุดในแอปตัวแรก

| | ใครเห็น | ใช้เมื่อ |
| --- | --- | --- |
| `require` ไม่ผ่าน | ไม่มีใคร | สิ่งที่ไม่ควรเป็นเท็จเลย ถ้าเป็นเท็จแปลว่าคุณมีบั๊ก |
| `verify` ไม่ผ่าน | ผู้ใช้ | credential ปลอม หมดอายุ หรืออยู่นอก anchor |
| `refuse "key"` | ผู้ใช้ | กติกาของคุณเอง: น้อยไป เร็วไป หรือรับไปแล้ว |
| host ปฏิเสธ | ผู้ใช้ | เขาตอบว่าไม่ |

`refuse` รับ key จาก `text.json` ไม่ใช่ประโยค และต้องอยู่ก่อน `execute`

### `update` เป็น patch

```val
update {
  lifetimePoints: total
  member.points:  state.member.points + earned
}
```

แต่ละบรรทัดคือ `path: value` อะไรที่ไม่ถูกระบุก็ไม่เปลี่ยน path ซ้อนกันได้แต่มี
index ของ list ไม่ได้ — ให้สร้าง list ใหม่ใน `compute` แล้วมาระบุชื่อที่นี่ใน
บรรทัดเดียว ผลลัพธ์ถูกผูกไว้ในชื่อ `next` ให้ `execute` อ่าน

### `execute` เป็นชุดเดียว

host เอา effect ไปทั้งหมดหรือไม่เอาเลย และ state ของคุณ commit ก็ต่อเมื่อมันเอาไป
ไม่มี effect ไหนอ่านผลของอีกอันได้ ถ้าอันหนึ่งขึ้นกับผลของอีกอัน นั่นคือสอง action

- **เปิดเผยได้อย่างมากหนึ่งครั้งต่อหนึ่ง action** การเปิดเผยย้อนกลับไม่ได้
  ครั้งที่สองจึงขึ้นกับชุดที่ครั้งแรกทำเสร็จไปแล้วไม่ได้ สองการเปิดเผยต้องการ
  ความยินยอมสองครั้ง ซึ่งก็คือสอง action
- **effect ที่ย้อนกลับไม่ได้ทำงานท้ายสุด** คอมไพเลอร์จัดลำดับให้เอง

### ฟังก์ชันเป็น pure

```val
function tierFor(points: int): Tier {
  return switch (points) {
    >= 10000 => Tier.gold,
    >= 2000  => Tier.silver,
    default  => Tier.bronze,
  }
}
```

ไม่มีฟังก์ชันที่มี effect effect จึงซ่อนอยู่หลังการเรียกไม่ได้

หมายเหตุ: ลำดับ effect ที่ใช้ร่วมกันสาม action ต้องเขียนออกมาสามครั้ง แลกกับการที่
ทุกอย่างที่ action ทำได้อยู่ในบล็อก `execute` ของมัน โดยไม่ต้องไล่กราฟการเรียก

---

## หน้าจอ

คุณประกาศหน้าจอ แล้ว wallet เป็นคนวาด

```val
screen Wallet {
  data {
    receipts: credentials of PurchaseReceipt verified with ReceiptFromMerchant
      order by purchased_at desc
      limit 50
  }

  compute {
    const totalValue = receipts.fold(0) { sum, r -> sum + r.claims.amount }
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

**ประกาศข้อมูล อย่าไปดึงเอง** host resolve บล็อก `data` ก่อนที่จะวาดอะไรทั้งสิ้น
`verified with` แปลว่า credential ที่ไม่ผ่าน policy ปรากฏไม่ได้ ไม่ใช่ "ถูกกรองออก"

`limit` จำเป็นสำหรับ list ที่คุณคำนวณต่อ มันจำกัดปริมาณงาน ซึ่งเป็นสิ่งที่ทำให้
ผลรวมของ list คอมไพล์ลงไปเป็นวงจรได้

**การกดระบุชื่อ action** `onTap` เป็น handler ชนิดเดียวที่มี ทุกอย่างที่หน้าจอ
เริ่มได้จึงผ่านหกเฟสด้วยความยินยอมชุดเดียวกันและ record ชุดเดียวกัน

**หน้าจออนุมานค่าได้ แต่ลงมือทำไม่ได้** บล็อก `compute` ของมันใช้กติกาเดียวกับ
action: pure ไม่มี effect

หมายเหตุ: เก็บผลรวมไว้ใน `compute` ไม่ใช่ใน `state` ค่าที่คำนวณได้จากสิ่งที่อยู่
บนหน้าจออยู่แล้วไม่จำเป็นต้องถูกเก็บ ถูกแฮช และถูก replay

**state ของการโต้ตอบเป็นของ host** แท็บไหนเปิดอยู่ ตำแหน่งการเลื่อน สิ่งที่พิมพ์
แต่ยังไม่ส่ง action ได้รับสิ่งที่ฟอร์มถืออยู่ ณ วินาทีที่มันถูกส่ง ผ่าน `input`

### Component

เป็นสิ่งที่ wallet ส่งมอบ ไม่ใช่สิ่งที่ภาษานิยาม:

```val
column { … }
section(text: "key")
card(text: phrase("key", name: value))
tile(text: phrase("key", name: value), onTap: Action)
list(binding) { item -> … }
button(text: "key", emphasis: primary, onTap: Action)
```

prop เป็นเชิงความหมาย — `text` `icon` `emphasis` `state` `onTap` ไม่มีสี
ไม่มีฟอนต์ ไม่มีขนาดพิกเซล การขอ component ที่ไม่มีในแคตตาล็อกจะถูกรายงาน
ไม่ใช่วาดของที่ใกล้เคียงออกมา package ของคุณบันทึกเวอร์ชันแคตตาล็อกที่มันถูก
build ด้วย และ host ก็เรนเดอร์ความหมายชุดนั้นหรือไม่ก็ปฏิเสธที่จะรันมัน

### ข้อความ

เขียนคำลงไปตรงๆ แอปที่มีภาษาเดียวไม่ต้องมี bundle เลย:

```val
section(text: "ใบเสร็จของคุณ")
card(text: phrase("คุณมี {points} แต้ม", points: state.member.points))
```

`phrase` ถือค่าที่จะเติม ส่วน host จัดรูปแบบตัวเลข วันที่ และสกุลเงินตามภาษาที่
กำลังใช้อยู่

ภาษาที่สองเปลี่ยนคำเหล่านั้นให้เป็น key:

```json
{
  "locales": ["en", "th"],
  "keys": {
    "balance": { "en": "You have {points} points", "th": "คุณมี {points} แต้ม" }
  }
}
```

```val
card(text: phrase("balance", points: state.member.points))
```

package ที่สัญญาสองภาษาแล้วยังเขียนคำลงไปตรงๆ คือ build ที่ล้มเหลว โดยบอกว่าคำนั้น
จะผิดในภาษาไหน เช่นเดียวกับช่องที่หายไป ช่องที่ผิด type และ key ที่ภาษาหนึ่งไม่ได้แปล

### ข้อมูลที่ไม่ใช่ credential

ราคา ข่าว แคตตาล็อก ดึงผ่าน host:

```val
data {
  holdings: credentials of Holding verified with FromLicensedBroker
  prices:   query broker.quotes(symbols: holdings.symbols) as List<Quote>
}
```

แอปของคุณยืนยันตัวตนด้วยการยื่น credential และ **ไม่เคยแตะโทเคน** host เป็นคนยื่น
ขอ access token ยิงคำขอ แล้วคืนแถวข้อมูลกลับมา

- ผู้รับสารถูกตรึงไว้ใน manifest ไม่เคยประกอบตอนรัน
- การได้สิทธิ์เข้าถึงเป็นการเปิดเผย ต้องประกาศ `disclosure.present`
- ผู้ใช้ยินยอมครั้งเดียวต่อหนึ่งแอปและหนึ่งผู้รับสาร ไม่ใช่ต่อการรีเฟรช
- host แคชคำตอบไว้และแสดงอายุของมัน
- เมื่อล้มเหลว คุณรู้ว่า query ไม่ตอบ แต่ไม่รู้ว่าเพราะอะไร

wallet แสดงข้อมูลสามระดับต่างกัน และคุณเลือกวิธีไม่ได้:

| ระดับ | มาจากไหน |
| --- | --- |
| ผู้ออกหนุนหลัง | claim ใน credential |
| ยืนยันด้วยตัวเอง | `state` ของคุณเอง |
| ต้นทางยืนยัน | query ที่ตอบโดย API ที่ยืนยันตัวตนแล้ว |

---

## การเปิดเผยและการพิสูจน์

```val
execute {
  present {
    disclose checked.claims.country
    prove checked.claims.birthdate <= context.time.now - duration(years: 20)
  }
}
```

`disclose` ยื่นค่าให้ ส่วน `prove` ยื่นคำตอบให้ — ผู้ตรวจสอบรู้ว่าคนคนหนึ่งอายุ
เกินยี่สิบ และคำนวณย้อนกลับไปหาวันเกิดไม่ได้

`prove` ผลิต `Proof<bool>` และไม่มีอะไรที่อ่อนกว่านั้น ในที่ที่ผลิต
zero-knowledge proof จริงๆ ไม่ได้ **แอปของคุณ build ไม่ผ่าน** มันไม่เคยถอยไป
เปิดเผยแล้วเปรียบเทียบแทน

### อะไรที่พิสูจน์ได้

`prove` คอมไพล์ลงไปเป็นวงจร และมีแค่ส่วนหนึ่งของภาษาที่ทำได้ คอมไพเลอร์บอกคุณ
ตอนที่คุณออกนอกส่วนนั้น

ภายใน: จำนวนเต็มที่ประกาศความกว้าง (`int<32>`) วันและเวลาที่เทียบแบบจำนวนเต็ม
การเท่ากันของ string `switch` และ `?:` list combinator ที่รู้ความยาวตอนคอมไพล์
nullifier ที่คำนวณจากความลับของผู้ถือ และการรวมอยู่แบบ Merkle เทียบกับรากของ state

ภายนอก: effect และอะไรก็ตามที่ไม่รู้ขนาดแบบสถิต

สองเรื่องที่ควรรู้ก่อนเขียน:

- **ทุกกิ่งมีราคา** วงจรจ่ายให้ทั้งสองข้างของเงื่อนไข
- **มันจ่ายตามขอบเขต ไม่ใช่ตามข้อมูล** proof เหนือ list ที่ยาวไม่เกิน 200
  จ่ายค่าบวก 200 ครั้ง ไม่ว่าผู้ใช้จะถือสองรายการหรือสองร้อยรายการ ซึ่งก็เป็น
  เหตุผลเดียวกับที่มันไม่รั่วว่าเขาถืออยู่กี่รายการ

### พิสูจน์เรื่อง state ของคุณเอง

state เป็น Merkle tree ฟิลด์เดียวจึงแสดงได้โดยไม่ต้องเปิดที่เหลือ:

```val
disclose state.member.tier
prove state.lifetimePoints >= 10_000
```

ให้รู้ว่าคุณกำลังอ้างอะไร claim ใน credential มีลายเซ็นของผู้ออกหนุนหลัง ส่วน
ฟิลด์ใน state มีห่วงโซ่ของ record ที่ผลิตมันหนุนหลัง — ถูกต้องตามกติกาที่ใครก็
รันซ้ำได้ แต่ไม่มีบุคคลที่สามอยู่เบื้องหลังข้อมูลนำเข้า ผู้ตรวจสอบถูกบอกว่ากำลังดู
อันไหนอยู่

**อะไรที่มาจาก API พิสูจน์ไม่ได้** คำตอบของ query เป็นคำพูดของใครคนหนึ่ง ไม่ใช่
ลายเซ็น คอมไพเลอร์ปฏิเสธมัน ให้เปิดเผยตัวเลขนั้นแล้วบอกว่ามันมาจากไหนแทน

---

## State และ execution record

`state` เป็นของคุณ อยู่บนเครื่องผู้ใช้ เปลี่ยนได้โดย `update` เท่านั้น

หลังทุก action host สร้าง Merkle tree เหนือใบ `(path, value)` ของ state คุณ
แล้วบันทึก **ราก** ไว้ record ถัดไปพารากก่อนหน้าไปด้วย ทั้งสองจึงต่อกันเป็นห่วงโซ่

นี่คือการ**ทำให้การแก้ไขปรากฏ** ไม่ใช่การกันการแก้ไข: state อยู่บนเครื่องของผู้ใช้
และเขาทิ้งมันได้ สิ่งที่ห่วงโซ่ให้คือการตรวจจับ สำหรับคนที่เก็บ record ก่อนหน้าไว้
— ซึ่งเป็นเหตุผลที่ผู้ตรวจสอบจำรากล่าสุดที่เห็นไว้ และเป็นเหตุผลที่ credential
ที่ออกไปบันทึกรากที่มันอนุมานมา

credential ถูกใช้ครั้งเดียว การย้อนกลับแล้วสแกนใบเสร็จเดิมซ้ำคือการใช้ซ้ำ และ
ผู้ออกปฏิเสธมันด้วยการบันทึก **nullifier** — คำนวณภายใน proof จากความลับของผู้ถือ
และตัวระบุของโครงการ มันจึงให้ค่าเดิมทุกครั้งสำหรับคู่นั้น และให้ค่าที่ไม่เกี่ยวข้อง
กันสำหรับคู่อื่น

### record บรรจุอะไร

ตัวระบุแอปและเวอร์ชัน ผู้เผยแพร่ แฮชของโค้ด action แฮชของ input รากของ state
ก่อนและหลัง policy ที่ใช้ capability ที่ใช้ effect ที่ขอและที่ทำไปแล้ว
(รวมถึงการเปิดเผย) บริบทตอนรัน ตราเวลา และลายเซ็น

### ทำให้ state เล็ก

- **ไม่เก็บค่าที่คำนวณได้** หน้าจอมี `compute` ให้แล้ว
- **ไม่เก็บ state ของการโต้ตอบ** นั่นเป็นของ host
- ขนาดถูกจำกัดโดย host และขีดจำกัดถูกตรวจก่อน state จะ commit

### การเปลี่ยนรูปร่างคือเวอร์ชันใหม่

**การเปลี่ยนรูปร่างของ `state` ทำให้ state ของเวอร์ชันนั้นเริ่มจากว่างเปล่า**
ไม่มี migration ไม่มี compatibility shim ไม่มี dual reader — migration คือโค้ดที่
รันกับข้อมูลที่เวอร์ชันปัจจุบันไม่เคยผลิต และ replay จาก record ไม่ได้ เพราะไม่มี
action ไหนเป็นคนทำ

อะไรที่เสียไปไม่ได้ควรอยู่ใน credential ที่คุณออกให้ หรือบน backend ของคุณเอง

---

## ความเป็น deterministic

ในตัวภาษาไม่มี `Date.now()` ไม่มี `random()` ไม่มี `fetch()` และไม่มีระบบไฟล์
ค่าที่ไม่แน่นอนมาจาก host และถูกบันทึกไว้:

```val
context.time.now      context.random.uuid
```

คำตอบของ query ที่ข้ามเข้าไปใน `compute` `update` `issue` หรือ `prove` ก็ถูก
บันทึกไว้ในบริบทตอนรันเช่นกัน การรันจึง replay ได้อยู่

ทุกโปรแกรมหยุด: กราฟการเรียกต้องไม่มีวัฏจักร และ list ถูกใช้ผ่าน combinator ที่มี
ขอบเขตเท่านั้น

---

## การทำ package

```bash
valc    file.val …             # diagnostics แล้วตามด้วยรายงาน capability
valrun  file.val ActionName    # รันหนึ่ง action แล้วพิมพ์ execution record
valpack build  ./dir -o app.va
valpack verify app.va
```

`.va` คือเอกสารที่ถูกเซ็นหนึ่งฉบับ: source ของคุณ manifest text bundle รายงาน
capability ที่อนุมานมา แฮชต่อไฟล์ และลายเซ็นครอบทั้งหมด input ชุดเดิมผลิตไบต์
ชุดเดิมเสมอ

**source เดินทางไปใน package** host จึงตรวจมันได้จากหลักการแรกแทนที่จะเชื่อ build
ของคุณ

### รายงาน capability

คอมไพเลอร์อนุมานมันจากโค้ดของคุณ คุณเขียนหรือแก้มันไม่ได้

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

ใบยินยอมที่ผู้ใช้กดอนุมัติคือการเรนเดอร์รายงานนี้ อ่านมันอย่างที่เขาจะอ่าน:
ถ้ามันบอกอะไรที่คุณไม่ได้ตั้งใจ แปลว่าโค้ดของคุณบอกอย่างนั้น

### host ตรวจอะไรก่อนรับ package ของคุณ

1. ทุก source แฮชออกมาตรงกับที่ integrity บอก
2. ลายเซ็นครอบไบต์ชุดนี้ โดยกุญแจที่ manifest ระบุชื่อ
3. มันคอมไพล์ผ่าน — ตรวจที่นั่น ไม่ได้รับมาจาก build ของคุณ
4. รายงานที่มันส่งมาคือรายงานที่โค้ดของมันผลิต
5. ทุกภาษาที่ manifest สัญญาไว้มีครบทุก key

จากนั้นเป็น policy ของ host เอง: แอปชนิดของคุณถือ capability เหล่านั้นได้ไหม
และมันเรนเดอร์เวอร์ชันแคตตาล็อกของคุณได้หรือเปล่า

### ใครเซ็น credential ที่คุณออก

ไม่ใช่แอปของคุณ มันไม่มีกุญแจผู้ออกและต้องไม่มี

```
เครื่องผู้ใช้    รัน action แล้วเซ็น execution record
   ↓
backend ของคุณ  ตรวจ record นั้น แล้วเซ็น credential ด้วยกุญแจผู้ออกของคุณ
   ↓
เครื่องผู้ใช้    เก็บ credential ไว้
```

เซิร์ฟเวอร์ของคุณไม่ได้รัน VAL ไม่ได้ถือ state และไม่เห็นมัน มันตรวจลายเซ็น
แปลงแฮชของโค้ดกลับไปเป็นเวอร์ชันที่คุณเผยแพร่ ตรวจห่วงโซ่ความเชื่อถือ ตรวจ proof
ถ้ามี — กุญแจสำหรับตรวจมาจากวงจรที่คอมไพล์แล้ว มันจึงรู้เองว่า predicate ไหนถูก
พิสูจน์ — แล้วจึงเซ็นหรือปฏิเสธ

คนที่ถือเครื่องอยู่เขียน state ของตัวเองใหม่ได้ แต่เขาทำให้เซิร์ฟเวอร์ของคุณเซ็น
credential ให้กับการรันที่ตรวจไม่ผ่านไม่ได้

---

## มันรันอย่างไร

```
.val sources
     │
     ├─ lexer → parser → typed AST
     ├─ การตรวจ type            Verified<P>, T?, ที่มา
     ├─ การวิเคราะห์ capability และความเชื่อถือ
     ├─ determinism และ totality
     │
     ├─ evaluator              เดิน typed AST
     └─ Wasm back end          สำหรับขีดจำกัดเชื้อเพลิงและ bytecode ที่ถูกเซ็น
```

back end ทั้งสองอ่าน typed AST ชุดเดียวกัน ไม่มี IR อยู่ระหว่างกลาง
Wasm back end เก็บค่าไว้ฝั่ง host แล้วส่ง handle เป็น `i32` จึงไม่ต้องมีตัวจอง
หน่วยความจำ และการ trap เมื่อจำนวนเต็มล้นก็แมปเข้ากับ trap ของ Wasm ตรงๆ
iOS ห้าม JIT runtime จึงเป็นแบบตีความ

### host จัดหาอะไรให้

VAL ไม่มี I/O ในที่ที่ต้องมี effect มันปล่อยคำอธิบายออกมา:

```
EffectRequest { capability, operation, payload }
```

host ตัดสินตามลำดับ: capability ถูกประกาศไหม · ผู้ใช้ยินยอมหรือยัง · policy ของ
host อนุญาตไหม · แอปน่าเชื่อถือไหม · ปฏิบัติการอยู่ในขอบเขตไหม

มันยังจัดหาการเข้ารหัสแบบ canonical (deterministic CBOR) นาฬิกาและความสุ่ม
การไล่ห่วงโซ่ความเชื่อถือ แคตตาล็อก component การจัดการ session และโทเคน
และขอบเขตของขนาดค่า

---

## อ้างอิง

### Builtin

เป็นชุดปิด แอปเพิ่มเข้าไปไม่ได้

```val
duration(days: 30)  duration(hours: 24)  duration(years: 20)
min  max  abs
```

### List combinator

```val
map  filter  fold  any  all  count  first
```

### Effect

อยู่ใน `execute` เท่านั้น ไม่เคยอยู่หลังฟังก์ชัน และถูกยื่นเป็นชุดเดียว

```val
credential.issue(Type { … })
payment.request(to: …, amount: …)
storage.write(scope: …, id: …, value: …)
message.send(to: …)
network.request(…)
present { disclose … / prove … }
navigate Screen
```

### นิพจน์

```val
const x = …                      // ไม่มี var ไม่มีการกำหนดค่า
a ? b : c                        // if เป็นคำสั่ง อันนี้เป็นนิพจน์
if (cond) { … } else { … }
switch (x) { A => 1, B => 2, }   // enum ไม่มี default
{ ...record, field: value }      // สร้างต่อยอด ไม่แก้ของเดิม
x with Policy                    // ทางเดียวที่จะได้ Verified<P>
x exists                         // ทำให้แคบลง ใช้ใน require
value from { Policy }            // ที่มา ใช้กับ claim ที่ออกไป
```

### ข้อมูลของหน้าจอ

```val
data {
  name: credentials of Type verified with Policy
    order by field desc
    limit 50

  other: query audience.operation(…) as List<Type>
}
```
