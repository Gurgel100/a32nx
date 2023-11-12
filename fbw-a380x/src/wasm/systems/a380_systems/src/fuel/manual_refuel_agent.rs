/*
            | A     | B     | C     | D     | E      | F      | G      | H      | I
Fuel Weight | 18000 | 26000 | 36000 | 47000 | 103788 | 158042 | 215702 | 223028 | 267764+
Note: +(varies with fuel density)

Outer Feed Mark A = 4500
Outer Feed Mark C = 7000
Outer Feed Mark E = 20558

Inner Feed Mark A = 4500
Inner Feed Mark C = 7000
Inner Feed Mark E = 21836

Outer Tank Mark B = 4000
Outer Tank Mark H = 7693

Mid Tank Mark F = 27127

Inner Tank Mark D = 5500
Inner Tank Mark G = 34300

Feed 1+4 Tanks
F14 = (WT / 4) if WT <= [Point A]
F14 = [Outer Feed Mark A] if [Point B] >= WT > [Point A]
F14 = [Outer Feed Mark A] + (WT - [Point B]) / 4 if [Point C] >= WT > [Point B]
F14 = [Outer Feed Mark C] if [Point D] >= WT > [Point C]
F14 = [Outer Feed Mark C] + (WT - [Point D]) * ([Outer Feed Mark E]/([Outer Feed Mark E] * 2 + [Inner Feed Mark E] * 2)) if [Point E] >= WT > [Point D]
F14 = [Outer Feed Mark E] if [Point H] >= WT > [Point E]
F14 = [Outer Feed Mark E] + (WT - [Point H]) / 10 if WT > [Point H]

Feed 2+3 Tanks
F23 = (WT / 4) if WT <= [Point A]
F23 = [Inner Feed Mark A] if [Point B] >= WT > [Point A]
F23 = [Inner Feed Mark A] + (WT - [Point B]) / 4 if [Point C] >= WT > [Point B]
F23 = [Inner Feed Mark C] if [Point D] >= WT > [Point C]
F23 = [Inner Feed Mark C] + (WT - [Point D]) * ([Inner Feed Mark E]/([Outer Feed Mark E] * 2 + [Inner Feed Mark E] * 2)) if [Point E] >= WT > [Point D]
F23 = [Inner Feed Mark E] if [Point H] >= WT > [Point E]
F23 = [Inner Feed Mark E] + (WT - [Point H]) / 10 if WT > [Point H]

Outer Tanks
O = 0 if WT < [Point A]
O = (WT - [Point A]) / 2 if [Point B] >= WT >= [Point A]
O = [Outer Tank Mark B] if [Point G] >= WT > [Point B]
O = [Outer Tank Mark B] + (WT - [Point G]) / 2 if [Point H] >= WT > [Point G]
O = [Outer Tank Mark H] + (WT - [Point H]) / 10 if WT > [Point H]

Mid Tanks
M = 0 if WT < [Point E]
M = (WT - [Point E]) / 2 if [Point F] >= WT >= [Point E]
M = [Mid Tank Mark F] if [Point H] >= WT > [Point F]
M = [Mid Tank Mark F] + (WT - [Point H]) / 10 if WT > [Point H]

Inner Tanks
I = 0 if WT < [Point C]
I = (WT - [Point C]) / 2 if [Point D] >= WT >=  [Point C]
I = [Inner Tank Mark D] if [Point F] >= WT > [Point D]
I = [Inner Tank Mark D] + (WT - [Point F]) / 2 if [Point G] >= WT > [Point F]
I = [Inner Tank Mark G] if [Point H] >= WT > [Point G]
I = [Inner Tank Mark G] + (WT - [Point H]) / 10 if WT > [Point H]
*/
