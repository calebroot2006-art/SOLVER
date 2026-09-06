# Evaluator notices

`cards` uses `rs_poker` 5.1.0 with default features disabled. Its published archive
contains the [Apache-2.0 license](rs_poker-Apache-2.0.txt). The archive SHA-256 is
`b773ee9ce87668d3842124279a523f70e89cb689c601e7d01b95a59a3c41d1a1`.

The dependency's `build.rs` identifies rank constants taken from OMPEval. Its
comment calls that project MIT, but the upstream notice is
[ISC](OMPEval-ISC.txt), copyright 2016 Timo A. We retain that notice verbatim from
[OMPEval commit 4aec210](https://github.com/zekyll/OMPEval/blob/4aec210ff75b0851af0ee170b35a7899e1a4fe8f/LICENSE.txt).
Its SHA-256 is `b57548ecdf94f00f2f4d06ca96f598c52eed3baaee9b4e2c81bf7ec9c02c4672`.

Include both notices in any distribution containing this evaluator. These files
cover the evaluator attribution reviewed in phase 2; they are not a complete
license inventory for the desktop app or the remaining dependency graph.
