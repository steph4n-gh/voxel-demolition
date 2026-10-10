# Third-party notices

The game depends on Macroquad and the packages pinned in `Cargo.lock`. The release
packaging script reads the host-target Cargo dependency graph and includes each
package's declared license, authors, source URL, original Cargo manifest, and
available license/notice files in `THIRD_PARTY_LICENSES/`. Its generated
`DEPENDENCIES.md` records the exact target and versions, including build dependencies.

Macroquad also embeds **ProggyClean.ttf**, by Tristan Grimmer, under the MIT
license. Its license is included separately. The font embedded by Macroquad
0.4.16 has Git blob SHA-1 `0270cdfe3caa811e1b252cdcc5b179989c1b8ad6`, identical to
the font in the [official Proggy fonts repository](https://github.com/bluescan/proggyfonts/tree/139ec08a38096161291792313ef5803fc4f0e37b/ProggyOriginal).

Some published crate archives omit standalone license texts. This repository
supplies the following notices for the pinned versions:

- `miniquad 0.4.11` also contains Apple utility code credited to Makepad. The
  [original Miniquad addition](https://github.com/not-fl3/miniquad/blob/18bdd4bae170def4f4f0a0cef4723e9c70ad36b9/src/native/apple/apple_util.rs)
  and the [pinned crate source](https://github.com/not-fl3/miniquad/blob/4f13d4a70caf6a8cbf0187a95461658e7812b566/src/native/apple/apple_util.rs)
  preserve that attribution. Four original helpers match the official Makepad
  [0.3.0 Apple utility source](https://github.com/makepad/makepad/blob/89300ddf4937f12b1152a26e2526be33af2e3780/platform/src/os/apple/apple_util.rs)
  after whitespace normalization. The corresponding [2021 Makepad MIT notice](https://github.com/makepad/makepad/blob/89300ddf4937f12b1152a26e2526be33af2e3780/LICENSE)
  is included alongside Miniquad's own licenses. Its exception for
  `studio/src/design_editor` does not cover these Apple utilities. The upstream
  license SHA-256 is
  `7e5e17be56687633db139b8223dc4119e9482123bbe164650d31f38c5048dc2d`;
  only line-end whitespace is normalized in the checked-in copy. This pre-copy
  snapshot establishes the notice; the credited source link does not identify
  the exact Makepad revision originally copied.

- `dasp_frame 0.11.0`: [original MIT notice](https://github.com/rustaudio/dasp/blob/221b81038c528bf3fc364a8ab5cc4b2e52f7dbfc/LICENSE-MIT)
  at the crate's recorded source commit.
- `dasp_sample 0.11.0`: [original MIT notice](https://github.com/rustaudio/sample/blob/97c3bb9b2363c0b46ac1633858bf1054fd02a980/LICENSE-MIT)
  at the crate's recorded source commit.
- `malloc_buf 0.0.6` and `quad-rand 0.2.3`: their package manifests declare MIT;
  the package archives do not contain standalone notices. The manifest attribution
  and the standard MIT terms are included without assigning additional copyright
  ownership. `quad-rand`'s [pinned manifest](https://github.com/not-fl3/quad-rand/blob/e110e57ea050a48c6a04d821fa3963c3b6d0f9d0/Cargo.toml)
  confirms the declaration; `malloc_buf`'s source is tagged
  [0.0.6](https://github.com/SSheldon/malloc_buf/tree/a7811e5f4c6f9685dd57ad9bfe34e9bf6b0ba4f9).
- `quad-snd 0.2.8`: its [pinned manifest](https://github.com/not-fl3/quad-snd/blob/5043a949ce02bc2119cfb7fb804ebec75ff71447/Cargo.toml)
  declares MIT/Apache-2.0 and supplies no standalone license texts. The release
  includes its manifest attribution and the standard Apache 2.0 terms.

The packaging process uses the Cargo cache and these checked-in supplements. It
does not download license files. A dependency change requires reviewing these
supplements and any new missing notices; the script rejects unknown missing
license texts.
