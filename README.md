A Git prompt for Zsh written in Rust using `git2`.

Because of `git2` limitation, only one state will be reported at a time, for
example: if a merge happens during bisect, the state reported will be "merge",
not "bisect and merge".
