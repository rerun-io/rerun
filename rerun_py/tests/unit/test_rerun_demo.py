import sys

import pytest
from rerun_demo import __main__ as main


# fail for any deprecation warning
pytestmark = pytest.mark.filterwarnings("error")


def test_run_cube():
    sys.argv = ["prog", "--cube", "--headless"]
    main.main()
