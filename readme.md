# My Collection Manager

This project is built as a companion project of the CosmWasm tutorials. Its object is to show various features of CosmWasm, along with the progression of the code as elements and features are added to demonstrate how a smart contract can communicate with the rest of the system.

The progression of the code is demonstrated via the help of branches and diffs.

## Progressive feature branches

The project is created with a clean list of commits in order to demonstrate the natural progression of the project. In this sense, there is no late commit that fixes an error introduced earlier. If there is a fix for an error introduced earlier, the fix should be squashed with the earlier commit that introduced the error. This may require some conflict resolution.

Having a clean list of commits makes it possible to do meaningful `compare`s.

To make it easier to link to the content at the different stages of the project's progression, a number of branches have been created at commits that have `Add branch the-branch-name.` as message. Be careful with the commit message as it depends on it matching the `"Add branch [0-9a-zA-Z\-]*\."` regular expression.

The script `push-branches.sh` is used to extract these commits and force push them to the appropriate branch in the repository. After having made changes, you should run this script, and also manually force push to `main`.

Versions used here are:

* Rust: 1.80.1 edition 2021

Branches:

* [`initial-pass-through`](../../tree/initial-pass-through)
* [`cross-contract-query`](../../tree/cross-contract-query), [diff](../../compare/initial-pass-through..cross-contract-query)
* [`reply-from-execute`](../../tree/reply-from-execute), [diff](../../compare/cross-contract-query..reply-from-execute)
