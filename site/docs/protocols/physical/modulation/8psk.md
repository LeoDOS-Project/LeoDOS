# 8PSK

Uses eight phases, carrying 3 bits per symbol. The phases are
assigned using Gray coding, meaning adjacent phase states differ by
only one bit --- so if noise causes the receiver to pick a
neighbouring phase, only one bit is wrong. Higher throughput, but
requires a stronger signal (better _link budget_ --- the overall
margin between transmitted power and minimum receivable power).
