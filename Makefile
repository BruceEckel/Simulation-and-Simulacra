# Simulation and Simulacra, from the command line.
#
#     make            what all of this does
#     make release    every simulation built and collected into dist/, one file each
#
# Adapted from the Makefile in the Fulcrum repository, which is where these pieces were written.
# What is different here is that the engine is not in this tree: it is a git dependency on
# jcerise/fulcrum, pinned in Cargo.lock, so there is a `make engine` for moving that pin on
# purpose and `--locked` on everything a release touches.
#
# Recipes run under PowerShell rather than a shell, because the `bash` on PATH here is WSL's, and
# a recipe run inside WSL would see a different filesystem and a different cargo. A `$` that
# PowerShell should see is written `$$`, because make eats the first one.
SHELL := pwsh.exe
.SHELLFLAGS := -NoProfile -Command
.DEFAULT_GOAL := help

# Every package under fulcrum/, which is the family of simulations built on that engine, plus
# `_viewer` — not a simulation but the front door onto them, named to sort first so that it is
# the first thing anybody sees in dist/. Nothing is excluded: unlike the repository these came
# from, there is no template in here.
SIMLIST = Get-ChildItem fulcrum -Directory

.PHONY: help sims build notes release dist run test check fmt lint engine publish publish-guards publish-upload clean

help: ## What all of this does
	@Write-Host ''
	@Write-Host 'Simulation and Simulacra' -ForegroundColor Cyan
	@Select-String -Path Makefile -Pattern '^([a-zA-Z-]+):.*?## (.*)$$' | ForEach-Object { $$m = $$_.Matches[0]; '  {0,-9} {1}' -f $$m.Groups[1].Value, $$m.Groups[2].Value }
	@Write-Host ''
	@Write-Host '  make run SIM=moebius3        one simulation, in release'
	@Write-Host '  make publish VERSION=v0.1.0  build here, tag, and upload the executables'
	@Write-Host ''
	@Write-Host 'The engine' -ForegroundColor Cyan
	@Write-Host ''
	@Write-Host '  jcerise/fulcrum, imported rather than copied and used unmodified: a git dependency'
	@Write-Host '  in Cargo.toml, with the commit it resolved to written down in Cargo.lock. A build'
	@Write-Host '  therefore does not drift under you, and a tag rebuilds a year from now into the'
	@Write-Host '  same executables. Anything the simulations need that the engine has not got is in'
	@Write-Host '  crates/ rather than in a fork of it.'
	@Write-Host ''
	@Write-Host '  make engine                  move the pin to the newest engine commit and test it'
	@Write-Host ''
	@Write-Host 'Publishing a release, in this order' -ForegroundColor Cyan
	@Write-Host ''
	@Write-Host '  1. make check                   format, clippy, tests, determinism: all clean'
	@Write-Host '  2. git commit -am "..."         nothing uncommitted; make publish refuses otherwise'
	@Write-Host '  3. git push                     the tag has to name a commit origin/main has'
	@Write-Host '  4. make release                 optional: build them and look at dist/ first'
	@Write-Host '  5. make publish VERSION=v0.1.0  builds, tags, pushes, and uploads dist/'
	@Write-Host ''
	@Write-Host '  Nothing builds this in the cloud. The set is compiled here and pushed up with the'
	@Write-Host '  gh CLI, which has to be installed and signed in, so what people download is what'
	@Write-Host '  you can run. Every simulation is one file with its assets compiled in, plus'
	@Write-Host '  SHA256SUMS.txt. Nothing to unzip.'
	@Write-Host ''
	@Write-Host '  Each executable goes up with its note from Windows/, so somebody who downloaded'
	@Write-Host '  one .exe can find out what it is and which keys it answers to. A simulation with'
	@Write-Host '  no note, or a note with no simulation, stops the release before it builds.'
	@Write-Host ''
	@Write-Host '  Versions are v-prefixed: v0.1.0, v0.2.0. Publishing a version whose tag still'
	@Write-Host '  names the commit you are on uploads to that same release again, which is how a'
	@Write-Host '  run that died partway is finished. A tag on any other commit is refused.'
	@Write-Host ''

sims: ## List the simulations
	@$(SIMLIST) | ForEach-Object Name

build: ## Debug build of everything
	@cargo build --workspace

# The notes that go up with the executables: one per simulation, written for whoever downloads
# one. They are uploaded beside the binaries because that is where they are needed. Somebody who
# has one `.exe` and no idea what it does or which keys it answers to should not have to find this
# repository to learn either.
NOTES = Windows

# --locked, because a release should be built from the dependency versions that are committed
# rather than from whatever resolved on the day. Here that covers the engine as well: the commit
# of Fulcrum this is built against is in Cargo.lock, so a release names one.
release: notes ## Every simulation in release, one file each into dist/, with its note and checksums
	@cargo build --workspace --release --locked
	@if (Test-Path dist) { Remove-Item dist -Recurse -Force }
	@New-Item -ItemType Directory dist | Out-Null
	@$(SIMLIST) | ForEach-Object { $$exe = "target/release/$$($$_.Name).exe"; if (-not (Test-Path $$exe)) { throw "$$($$_.Name) produced no executable" }; Copy-Item $$exe dist; Copy-Item "$(NOTES)/$$($$_.Name).md" dist }
	@Copy-Item $(NOTES)/README.md dist/NOTES.md
	@Get-ChildItem dist/*.exe | ForEach-Object { '{0}  {1}' -f (Get-FileHash $$_ -Algorithm SHA256).Hash.ToLower(), $$_.Name } | Set-Content dist/SHA256SUMS.txt -Encoding ascii
	@Write-Host ''
	@Write-Host ('  ' + (Get-ChildItem dist/*.exe).Count + ' executables in dist/, ' + [math]::Round(((Get-ChildItem dist/*.exe | Measure-Object Length -Sum).Sum/1MB),0) + ' MB, ' + (Get-ChildItem dist/*.md).Count + ' notes, and SHA256SUMS.txt') -ForegroundColor Green
	@Write-Host '  Each carries its own assets, so any one of them can be sent to somebody on its own.'
	@Write-Host ''

# Run before the build rather than after it, because being told a note is missing is worth knowing
# before a full release build rather than at the end of one. A simulation with no note reaches
# somebody as an executable with no way to find out what it is, which is the whole reason the
# notes go up in the first place; a note with no simulation is a file about something that is not
# in the release, which is worse than nothing.
notes: ## Check that everything under fulcrum/ has a note, and every note something to describe
	@$$sims = @($(SIMLIST) | ForEach-Object Name); $$notes = @(Get-ChildItem $(NOTES)/*.md | ForEach-Object BaseName | Where-Object { $$_ -ne 'README' }); $$missing = $$sims | Where-Object { $$_ -notin $$notes }; $$orphan = $$notes | Where-Object { $$_ -notin $$sims }; if ($$missing) { throw "no note in $(NOTES)/ for: $$($$missing -join ', ')" }; if ($$orphan) { throw "a note in $(NOTES)/ for something that is not built here: $$($$orphan -join ', ')" }; Write-Host ('  ' + $$sims.Count + ' executables, ' + $$notes.Count + ' notes, one each way')

# One line, because make gives every line its own shell: an `exit 0` in the first would not stop
# the second from running and failing on the directory that is not there.
dist: ## What is in dist/ at the moment
	@if (-not (Test-Path dist)) { Write-Host 'nothing built yet: make release' } else { Get-ChildItem dist/*.exe | Select-Object Name, @{n='MB';e={[math]::Round($$_.Length/1MB,1)}}, LastWriteTime | Format-Table -AutoSize }

run: ## One simulation, in release (SIM=...)
	@if (-not '$(SIM)') { Write-Host 'usage: make run SIM=moebius3'; Write-Host 'names: make sims'; exit 1 }
	@cargo run -p $(SIM) --release

test: ## The workspace test suite
	@cargo test --workspace

# The whole gate, and it runs here rather than in the cloud. The last two lines are the
# determinism promise: the seeded runs and the replay round-trips, in release, which is the build
# that ships. Floating-point work can differ between debug and release, so a gate that only ran in
# debug would be watching the wrong arithmetic.
check: ## The whole gate: format, clippy, tests, and determinism in release
	@cargo fmt --all --check
	@cargo clippy --workspace --all-targets -- -D warnings
	@cargo test --workspace
	@cargo test --workspace --release determinism
	@cargo test --workspace --release replay
	@Write-Host ''
	@Write-Host '  format, clippy, the tests, and the determinism gate in release: all clean' -ForegroundColor Green
	@Write-Host ''

fmt: ## Format everything
	@cargo fmt --all

lint: ## Clippy, warnings as errors
	@cargo clippy --workspace --all-targets -- -D warnings

# Moving the engine pin is a deliberate act with a test run attached, not something a build does
# behind you. `cargo update -p fulcrum` walks the pin to the newest commit on the engine's default
# branch; everything after it is there so that a pin only moves once the whole set still builds and
# still passes on the new one. If it does not, `git checkout Cargo.lock` puts you back.
engine: ## Move the engine pin to the newest Fulcrum commit, then build and test against it
	@$$was = (Select-String -Path Cargo.lock -Pattern 'jcerise/fulcrum.git#(\w+)').Matches[0].Groups[1].Value; Write-Host ('  was ' + $$was)
	@cargo update -p fulcrum
	@$$now = (Select-String -Path Cargo.lock -Pattern 'jcerise/fulcrum.git#(\w+)').Matches[0].Groups[1].Value; Write-Host ('  now ' + $$now)
	@cargo build --workspace
	@cargo test --workspace
	@Write-Host ''
	@Write-Host '  the engine pin has moved and everything still builds and passes.' -ForegroundColor Green
	@Write-Host '  Commit Cargo.lock to keep it; git checkout Cargo.lock to put it back.'
	@Write-Host ''

# Build here, then publish what was built. The three steps run in this order because the guards
# are cheap and the build is not: being told the tree is dirty is worth knowing before a full
# release build rather than after one.
publish: publish-guards release publish-upload ## Build here, tag, and upload the executables (VERSION=...)

# The guards are the published order, enforced. Getting these wrong is not a build failure, it is a
# release that quietly went out from the wrong commit.
publish-guards:
	@if (-not '$(VERSION)') { Write-Host 'usage: make publish VERSION=v0.1.0'; exit 1 }
	@if ('$(VERSION)' -notmatch '^v[0-9]') { Write-Host 'versions are v-prefixed: v0.1.0'; exit 1 }
	@if (-not (Get-Command gh -ErrorAction SilentlyContinue)) { Write-Host 'the release is uploaded with gh, which is not installed: https://cli.github.com'; exit 1 }
	@gh auth status *> $$null; if ($$LASTEXITCODE -ne 0) { Write-Host 'gh is not signed in to GitHub: gh auth login'; exit 1 }; exit 0
	@if (@(git status --porcelain).Count -gt 0) { Write-Host 'there is uncommitted work here; commit it first'; exit 1 }
	@git fetch origin --quiet
	@if ([int](git rev-list --count origin/main..HEAD) -gt 0) { Write-Host 'origin does not have these commits yet; git push first, or the tag names a commit nobody else can see'; exit 1 }
	@if (@(git tag --list '$(VERSION)').Count -gt 0 -and (git rev-parse '$(VERSION)^{commit}') -ne (git rev-parse HEAD)) { Write-Host '$(VERSION) already names a different commit; pick the next version'; exit 1 }

# Tagged only once the build has come out whole, so a build that fails leaves nothing behind to
# undo. Everything here is safe to run twice against the same version: the tag is only made if it
# is missing, the release only if it is missing, and the upload clobbers what is already up. That
# is the recovery path for a publish that died halfway, and it is the only one there is now that
# nothing rebuilds this in the cloud.
publish-upload:
	@if (@(git tag --list '$(VERSION)').Count -eq 0) { git tag $(VERSION) }
	@git push origin $(VERSION)
	@gh release view $(VERSION) *> $$null; if ($$LASTEXITCODE -ne 0) { gh release create $(VERSION) --title $(VERSION) --generate-notes; if ($$LASTEXITCODE -ne 0) { throw 'could not create the release for $(VERSION)' } }; exit 0
	@gh release upload $(VERSION) (Get-ChildItem dist/* | ForEach-Object FullName) --clobber
	@Write-Host ''
	@Write-Host '  $(VERSION) is published, with everything in dist/ attached to it.' -ForegroundColor Green
	@Write-Host ('  ' + (gh release view $(VERSION) --json url --jq .url))
	@Write-Host ''

clean: ## Remove dist/ and everything cargo built
	@if (Test-Path dist) { Remove-Item dist -Recurse -Force }
	@cargo clean
