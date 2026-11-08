# Installateurs Dock Windows

Chaque version publiée de l'application est déposée ici sous la forme d'un
paquet MSI x64 installable par utilisateur, accompagné de son empreinte SHA-256.

## Version courante : 1.32.9

- [DockWindows-1.32.9-x64.msi](https://github.com/xev47/DockWindows/raw/Version-1.32.9/releases/DockWindows-1.32.9-x64.msi) — installateur Windows 10/11 x64.
- [DockWindows-1.32.9-x64.msi.sha256](https://github.com/xev47/DockWindows/raw/Version-1.32.9/releases/DockWindows-1.32.9-x64.msi.sha256) — empreinte de vérification.

Vérifier le fichier téléchargé :

```powershell
Get-FileHash .\DockWindows-1.32.9-x64.msi -Algorithm SHA256
```

Installer en double-cliquant le MSI, ou en silence :

```powershell
msiexec /i .\DockWindows-1.32.9-x64.msi /qn
```

La configuration personnelle (`%APPDATA%\DockWindows`) est conservée lors d'une
mise à jour ; elle peut aussi être exportée et importée depuis le panneau des
paramètres.

Le binaire est construit depuis les sources de ce dépôt (`rust-dock/`) par
`rust-dock/installer/build.ps1`.

Seule la dernière version est conservée dans ce dossier.
