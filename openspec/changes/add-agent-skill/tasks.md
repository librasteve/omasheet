# Tasks: Ship the Omasheet skill with the package

## 1. Skill in the repository
- [x] 1.1 Move `.claude/skills/omasheet/SKILL.md` to `skill/omasheet/SKILL.md`
- [x] 1.2 Leave `.claude/skills/omasheet` as a symlink to it
- [x] 1.3 `README.md`: the raw URL points at `skill/omasheet/SKILL.md`, with a note on the link command
- [x] 1.4 `SKILL.md`: offer the package or `cargo install` where `omasheet` is not on the PATH

## 2. Package (`packaging`)
- [x] 2.1 Both `PKGBUILD`s install the skill to `/usr/share/omasheet/agent-skill/omasheet/SKILL.md`
- [x] 2.2 Both `.install` files print the `ln -sfn` command on install and upgrade, and not on removal
- [ ] 2.3 Build the package with `pkgbuild/build` and confirm the skill is in it
- [ ] 2.4 Install it and confirm the hint prints on install and upgrade, and not on removal
- [ ] 2.5 Run the printed `ln -sfn` and confirm Claude Code loads the skill

## 3. Release
- [ ] 3.1 Bump `pkgver` to 0.1.8 in both `PKGBUILD`s
- [ ] 3.2 Merge `dev` to `main`, so the README's raw URL resolves
- [ ] 3.3 Tag `v0.1.8`
- [ ] 3.4 `omarchy-pkgs/omasheet/PKGBUILD`: update `sha256sums` for the tag's tarball

## 4. Tidy
- [x] 4.1 Point the `openspec/changes/**` mentions of the old skill path at the new one
