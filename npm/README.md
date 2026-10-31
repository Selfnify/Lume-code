# Lume Code

A terminal coding assistant connected to your Lume account. Work with local files, review commands before running them, plan changes, and resume saved conversations.

## Install

Requires Windows x64 and Node.js 20 or newer. Rust is not required to install or use the published package.

```powershell
npm install --global lume-code
lume
```

Start from your project folder, or select one explicitly:

```powershell
lume --project "C:\path\to\project"
```

Sign in through your browser with your Lume account. If the browser does not open automatically, copy the URL shown in the terminal. Your account credential is stored in Windows Credential Manager.

```powershell
lume login
lume logout
lume --help
lume --version
```

Type `/` inside Lume for commands, `?` for keyboard shortcuts, or use Shift-Tab to switch between Plan and Edit. Local commands require your approval. Review commands carefully because they run with your Windows user permissions.

Lume Code uses the public Lume API and shares your account's usage allowance. A network connection and a Lume account are required. No backend installation is needed.

Update with `npm install --global lume-code@latest`. Remove with `npm uninstall --global lume-code`.
