## Hooks
This folder contains the official rerun githooks.

Each hook is designed to call through to a corresponding hook in the scripts directory.
 - `pre-push` -> `scripts/pre-push.sh`

### Installation
To install the hooks, simply copy them into the `.git/hooks` directory of your local checkout.
```
cp hooks/pre-push .git/hooks/pre-push
chmod +x .git/hooks/pre-push
```
or if you prefer you can configure git to use this directory as the hooks directory:
```
git config core.hooksPath hooks
```
