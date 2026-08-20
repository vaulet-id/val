# อ้างอิง

ทุกอย่างที่ภาษานี้มี รวมไว้ที่เดียว กติกาเบื้องหลังแต่ละข้ออยู่ใน
[เอกสารอ้างอิง](../../th/spec.md)

## Type

| | |
| --- | --- |
| `int` | 64 บิต มีเครื่องหมาย **trap** เมื่อล้นและเมื่อหารด้วยศูนย์ |
| `string` | เทียบและส่งต่อได้ ประกอบไม่ได้ ไม่มี interpolation ไม่มี `+` |
| `bool` | `true`, `false` |
| `date`, `datetime` | เทียบกันแบบจำนวนเต็ม บวก duration เข้าไปแล้วยังเป็นชนิดเดิม |
| `bytes` | |
| `List<T>` | ไม่มี index ใช้ผ่าน combinator |
| `T?` | ทำให้แคบลงด้วย `exists` ใน `require` |
| `Credential<T>` | ถืออยู่ ยังไม่ตรวจ claim ของมันเอื้อมไม่ถึง |
| `Verified<P>` | สิ่งที่บล็อก `verify` ผลิต ระบุชื่อ **policy** |
| `Proof<bool>` | สิ่งที่ `prove` ผลิต |

ไม่มี floating point ที่ไหนทั้งสิ้น เงินเป็นหน่วยย่อย เปอร์เซ็นต์เป็น basis point

## การประกาศ

```val
app "reverse.dns.name"
version 1
capabilities { … }
enum Name { a, b }
credential Name { field: type }
type Name { field: type }        // record ธรรมดา ไม่มีใครเซ็นมัน
state { field: type default … }
trust Name(subject: Type) [refines Other] { anchor: "…" require { … } }
function name(a: int, b: int): int { … }
action Name { … }
component Name(a: string, b: int default 0) { … }
export component Name(…) { … }   // สิ่งที่ออกจาก package
import "other.package/1" { Name }
@main                            // หน้าจอที่แอปเปิดขึ้นมา
screen Name(param: string) { … }
```

## นิพจน์

```val
const x = …                      // นิยาม
let x = …                        // ตัวแปร เขียนซ้ำด้วย `x = …`
a ?: b                           // a เว้นแต่มันไม่มี
a?.b                             // ไม่มีค่า ถ้า a ไม่มี
{ a, b } = record                // ตามหลัง `const` หรือ `let`
0...10                           // รวมปลายทั้งสองข้าง
`words ${value} words`           // phrase ที่ host เป็นคนเติม
a ? b : c                        // if เป็นคำสั่ง อันนี้เป็นนิพจน์
if (cond) { … } else { … }
switch (x) { A => 1, B => 2, }   // enum ไม่มี default กิ่งที่ไปไม่ถึงเป็น error
{ ...record, field: value }      // สร้างต่อยอด ไม่เคยแก้ของเดิม
x with Policy                    // ทางเดียวที่จะได้ Verified<P>
x exists                         // ทำให้แคบลง ใช้ใน require
value from { Policy }            // ที่มา ใช้กับ claim ที่ออกไป
```

อาร์กิวเมนต์ระบุชื่อเมื่อมีตั้งแต่สองตัวขึ้นไป: `f(a: 1, b: 2)` บล็อกที่ต่อท้าย
เป็นบล็อก ไม่ใช่อาร์กิวเมนต์

## Builtin

เป็นชุดปิด แอปเพิ่มเข้าไปไม่ได้ — builtin เป็นที่เดียวที่ปฏิบัติการซึ่งไม่จบ
จะเล็ดลอดเข้ามาในภาษาที่พิสูจน์แล้วว่ามีสิ่งนั้นไม่ได้

```val
duration(days: 30)  duration(hours: 24)  duration(years: 20)
min  max  abs
```

## List combinator

```val
map  filter  fold  any  all  count  first
```

มีขอบเขตเท่ากับ list ที่มันเดิน ไม่มี recursion ไม่มี index และ `for` บนหน้าจอวนบน list
ที่ host ตอบมา หรือ range ที่เขียนความยาวไว้ —
ทุกโปรแกรมหยุด และคอมไพเลอร์รู้

## เฟส

```
input → require → verify → compute → update → execute
```

จะละอันไหนก็ได้ ห้ามสลับลำดับ `refuse "key"` ใช้ได้ก่อน `execute` และใช้ในนั้นไม่ได้

## Effect

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

## Component ของหน้าจอ

เป็นสิ่งที่ host ส่งมาให้ ไม่ใช่สิ่งที่ภาษานิยาม บน Vaulet วันนี้:

```val
column { … }
section(text: "key")
card(text: phrase("key", name: value))
tile(text: phrase("key", name: value), onTap: Action)
list(binding) { item -> … }
button(text: "key", emphasis: primary, onTap: Action)
```

prop เป็นเชิงความหมาย การขอ component ที่ไม่มีในแคตตาล็อกจะไม่ได้ของที่วาดใกล้เคียง
— แต่จะถูกรายงานออกมา

สองอย่างที่เขียนในต้นไม้เป็นของภาษาเอง และ host ไม่เคยเห็นทั้งคู่: กิ่งถูกเลือก
และลูปถูกคลี่ออก ก่อนที่จะวาดอะไร

```val
if (cond) { … } else { … }
for (row in rows) { … }
for (i in 1...10) { … }
```

## ข้อมูลของหน้าจอ

```val
data {
  name: credentials of Type verified with Policy
    order by field desc
    limit 50

  other: query audience.operation(…) as List<Type>
}
```

## บริบทตอนรัน

```val
context.time.now      context.random.uuid
```

เป็นแหล่งความไม่แน่นอนเพียงสองแหล่ง และถูกบันทึกทั้งคู่ `Date.now()` และ
`random()` ไม่มีอยู่

## เครื่องมือ

```bash
valc    file.val …             # diagnostics แล้วตามด้วยรายงาน capability
                               # อ่าน text.json ที่วางอยู่ข้าง source
valrun  file.val ActionName    # รันหนึ่ง action แล้วพิมพ์ execution record
valpack build  ./dir -o app.va
valpack verify app.va
```
