# Tasks: List Omasheet in OmaStore

## 1. Manifest (`packaging`)
- [x] 1.1 `omastore.toml`: name, summary, `Office`/`Spreadsheet`, icon, and the versioned release asset per architecture
- [x] 1.2 Check it with `omastore lint-manifest .` and against the real `v0.1.5` assets
- [x] 1.3 Commit it to `main` — the store reads the default branch, nowhere else
- [x] 1.4 Confirm with `omastore check librasteve/omasheet`: compatible, installable on both architectures

## 2. Provenance
- [x] 2.1 `package.yml`: attest the attached packages in the `attach` job, with `id-token: write` and `attestations: write`
- [x] 2.2 Narrow the workflow's top-level permission to `contents: read`
- [ ] 2.3 Publish a release with the workflow and confirm `omastore check` no longer warns about provenance

## 3. Discovery
- [x] 3.1 Add the `omarchy` topic, so the store finds the repository without a GitHub token
- [ ] 3.2 `README.md`: link `omastore://librasteve/omasheet`

## 4. Verify
- [ ] 4.1 Install Omasheet from OmaStore on a clean machine and open an `.omx` file from the launcher
