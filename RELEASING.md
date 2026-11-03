# Como publicar uma nova versão

Cada versão é publicada a partir de uma tag `vX.Y.Z`. O GitHub Actions compila o
`clearshot.exe` e anexa o `clearshot-windows-x64.zip` na página de Releases.

## Passo a passo

1. **Atualize a versão** em `Cargo.toml`:
   ```toml
   version = "0.2.0"
   ```

2. **Teste localmente** (isso também atualiza o `Cargo.lock`):
   ```
   cargo build --release
   target\release\clearshot.exe
   ```

3. **Faça o commit e envie**:
   ```
   git add .
   git commit -m "ClearShot v0.2.0"
   git push
   ```

4. **Crie e envie a tag**. É ela que dispara o build da release:
   ```
   git tag v0.2.0
   git push origin v0.2.0
   ```

5. **Confira** a aba **Actions** até o build terminar (5 a 10 min). Depois, abra
   **Releases**, edite a release criada e escreva o que mudou.

## Como escolher o número

- `0.1.0 → 0.1.1`: só correções de bugs
- `0.1.0 → 0.2.0`: novos recursos
- `0.9.0 → 1.0.0`: mudança grande ou versão considerada estável

O número da tag deve ser igual ao de `Cargo.toml`, com o `v` na frente.

## Se o build falhar

Corrija o problema, faça o commit e recrie a tag no novo commit:
```
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0
git tag v0.2.0
git push origin v0.2.0
```
