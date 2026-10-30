Releases for the tests, as GitHub's API gives them (`release.json`), with
their files: `DNT_UPDATE_URL` points at one. `older` (the tests' default)
is older than any dnt: nothing to update; `newer` is a newer one; `broken`
has a file that does not match its `SHA256SUMS`. The files are stand-ins,
not programs.
