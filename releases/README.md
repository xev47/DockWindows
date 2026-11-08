# Installateurs Dock Windows

Chaque version publiée de l'application est déposée ici sous la forme d'un
paquet MSI x64 installable par utilisateur, accompagné de son empreinte SHA-256.

## Version courante : 1.31.1

- `DockWindows-1.31.1-x64.msi` — installateur Windows 10/11 x64.
- `DockWindows-1.31.1-x64.msi.sha256` — empreinte de vérification.

Cette version corrige l'affichage du premier menu contextuel, qui pouvait être
rogné par la fenêtre du dock.

Au premier démarrage, un assistant permet de choisir la langue du dock
(français, anglais ou espagnol) et d'importer les raccourcis de la barre des
tâches, du bureau et du menu Démarrer.

Vérifier le fichier téléchargé :

```powershell
Get-FileHash .\DockWindows-1.31.1-x64.msi -Algorithm SHA256
```

Installer en double-cliquant le MSI, ou en silence :

```powershell
msiexec /i .\DockWindows-1.31.1-x64.msi /qn
```

La configuration personnelle (`%APPDATA%\DockWindows`) est conservée lors d'une
mise à jour ; elle peut aussi être exportée et importée depuis le panneau des
paramètres.

Le binaire est construit depuis les sources de ce dépôt (`rust-dock/`) par
`rust-dock/installer/build.ps1`.

## Versions précédentes

- `DockWindows-1.31.0-x64.msi` — installateur Windows 10/11 x64.
- `DockWindows-1.31.0-x64.msi.sha256` — empreinte de vérification.
- `DockWindows-1.30.0-x64.msi` — installateur Windows 10/11 x64.
- `DockWindows-1.30.0-x64.msi.sha256` — empreinte de vérification.
- `DockWindows-1.29.0-x64.msi` — installateur Windows 10/11 x64.
- `DockWindows-1.29.0-x64.msi.sha256` — empreinte de vérification.
