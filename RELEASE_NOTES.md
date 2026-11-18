## NyashkPre 1.0.11

- Нові кнопки: клавіша сидить у заглибленні панелі, опукла матова, при ввімкненні світиться зсередини, а світло м'яко лягає на панель навколо (раніше світіння обрізалось прямокутною рамкою).

## NyashkPre 1.0.10

- Шкали, риски, LED і підписи тепер згладжені (малюються в 4 рази більшими й зменшуються), без кривих країв.
- Світіння адитивне, як у справжніх LED: м'який ореол і яскраве ядро.
- Плавні анімації: кнопки загоряються й гаснуть, LED-кільця ковзають за ручкою, обідок навколо ручки плавно проявляється.
- Оптимізація: оновлення екрана 30 разів на секунду (Opto було 60), анімації не додають зайвих перемальовувань.

## NyashkPre 1.0.9

- Деталі дизайну: стрілка ручки починається від краю ковпачка (як справжня гравіровка), бокові стінки металевих ручок ловлять світло, навколо ручки, яку крутиш або на яку наводиш, світиться тонкий обідок, LED-кільця без зайвих рисок на маленьких ручках, мітки пікового рівня на LED-шкалах, більші підписи.

## NyashkPre 1.0.8

- Новий дизайн: сучасне аналогове залізо. Корпус, ручки, кнопки й екрани відрендерені як справжні фізичні об'єкти: точені алюмінієві ручки з рифленою стінкою, тіні в три шари, анодований метал з подряпинами, пилом і потертими краями, кнопки з підсвіткою, LED-дуги навколо ручок, скло на екранах.
- Ручки: тягни вгору / вниз (Shift — точно), колесо миші, подвійний клік — значення за замовчуванням, правий клік — ввести число. Значення показується при наведенні.

## NyashkPre 1.0.7

- Оверсемплінг тепер рівний до 20 кГц на 44.1 кГц (новий поліфазний half-band фільтр, дзеркала > 100 дБ нижче). Раніше на 44.1 кГц верх був приглушений: −0.6 дБ на 18 кГц і ≈ −5 дБ на 20 кГц.
- Захист від NaN / inf від хоста: один «битий» семпл більше не ламає плагін назавжди (раніше фільтри залишались у NaN і звук пропадав до перезапуску).

## NyashkPre 1.0.6

- The Drive on / off fade is S-shaped (no corner at its start or end).

## NyashkPre 1.0.5

- **Parallel Mix fixed:** the dry part of the mix skipped the oversampling filters the saturated part goes through, so the two were out of phase in the highs - a 50 % mix with Drive on dipped 8 dB around 6 kHz. Both now take the same path; the mix is flat (within 0.01 dB) from 1 to 16 kHz.
- **Drive on / off without clicks:** turning Drive up from 0 or back to 0 switched between two different signal paths at once (a click and a jump in the top end). The path stays the same now and the saturation fades in / out over 10 ms.
- New behaviour tests: distortion rises steadily with Drive for every character (Tube up to ~5 %, Iron ~3.5 %, Clean ~2.3 % at -12 dBFS), the tone controls sound exactly like the drawn curve, Auto Level keeps the level within ~2 dB from 0 to +24 dB gain.

## NyashkPre 1.0.4

- Level scale shows a plain 0 instead of +0.

## NyashkPre 1.0.3

A vocal preamp again, not a fuzz.

- **Gentler, bounded saturation:** Drive now spans clean -> warm the way a vocal preamp does. At the defaults (Gain +6 dB, Drive 30 %, Tube) a voice at -12 dBFS gets ~2.7 % THD, mostly 2nd harmonic (was ~11 % with a strong 3rd); 100 % Drive on hot peaks stays bounded (was over 100 % THD on Tube - the curve folded over).
- **Tube:** a biased soft curve - smooth 2nd harmonic first, odd ones only when pushed.
- **Less harshness on S and breaths:** the top end (above ~4.5 kHz) goes into the saturation 6 dB quieter and is lifted back after it, so sibilants make far fewer gritty high harmonics; the tone is unchanged.
- **Auto Level** now takes the whole Gain back off at the output (was a bit over half), so Gain only decides how hard the stage is hit.

## NyashkPre 1.0.2

- **Alt-tab fix (Linux):** after alt-tab, minimising or covering the window, parts of the picture (backgrounds, knobs, images) could stay blank until something on them moved. The window is now drawn again in full whenever the system throws its contents away, and pictures are no longer lost while the window is hidden or a frame is skipped.
- Smoother images in the CPU renderer (used when Vulkan is not available): pictures drawn smaller than their real size, such as knobs and photos, are now scaled with mipmaps instead of picking single pixels, so they no longer look grainy or jagged.
- **Display fix:** the built-in CPU renderer (used when Vulkan is not available) drew pixels on the shared edges of triangles twice, which showed as brighter seams and specks in see-through fills (spectra, curves, bars). It now uses an exact fill rule like a GPU: every pixel is drawn once, without holes.
- The "harmonics vs signal" label no longer runs into the OUT meter.
- Value boxes have a small gap between them, and a value that would not fit its box is drawn smaller instead of running over the frame.
- Clicking a value box and pressing Enter no longer changes a value whose unit matters (for example "1.03 s" was read back as 1.03 ms).
- No more "-0.0" read-outs. (Warmth, Presence, Silk)
- New automatic test: every parameter's text is typed back over its whole range and must give the same value.

## NyashkPre 1.0.1

- **Live spectrum** behind the tone curve.
- **Live trace on the saturation curve:** the newest samples light up on the curve, so you see how hard the preamp is being driven.

## NyashkPre 1.0.0

First release. VST3 for Linux, Windows and macOS.
