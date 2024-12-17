![Rerun.io](https://user-images.githubusercontent.com/1148717/218142418-1d320929-6b7a-486e-8277-fbeef2432529.png)

# Interactive release checklist
Welcome to the release checklist.

Run the testlist with:
```
pixi run -e examples python tests/python/release_checklist/main.py
```

### When releasing
Each check comes in the form a recording that contains:
1. a markdown document specifying the user actions to be tested, and
2. the actual data required to test these actions.

To go through the checklist, simply check each recording one by one, and close each one as you go if
everything looks alright.

If you've closed all of them, then things are in a releasable state.


### When developing
Every time you make a PR to add a new feature or fix a bug that cannot be tested via automated means
for whatever reason, take a moment to think: what actions did I take to manually test this, and should
these actions be added as a new check in the checklist?

If so, create a new recording by creating a new `check_something_something.py` in this folder.
Check one of the already existing ones for an example.

Each recording/check has a dedicated file that gets called from `main.py`; that's it.
