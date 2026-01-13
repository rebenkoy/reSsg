# ressg-helper README
## Features

Runs reSsg server in background and provides babushka-friendly "Save" button that just does 
`git branch "{branch_name}" && git add * && git commit -am "{commit_message}" && git push --set-upstream origin "{branch_name}"`
## Extension Settings

This extension contributes the following settings:

* `myExtension.enable`: Enable/disable this extension.

## Known Issues

This extension is tailored to my exact usecase, I do not plan to support anything else, at least right now.

## Release Notes

### 1.0.0

Initial release of ressg-vscode-extension
