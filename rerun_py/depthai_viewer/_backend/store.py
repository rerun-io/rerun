from enum import Enum
from typing import Callable, Dict, List, Optional, Tuple

from depthai_viewer._backend.device_configuration import PipelineConfiguration
from depthai_viewer._backend.topic import Topic


class Action(Enum):
    UPDATE_PIPELINE = 0
    SELECT_DEVICE = 1
    GET_SUBSCRIPTIONS = 2
    SET_SUBSCRIPTIONS = 3
    GET_PIPELINE = 4
    RESET = 5  # When anything bad happens, a reset occurs (like closing ws connection)
    GET_AVAILABLE_DEVICES = 6


class Store:
    pipeline_config: Optional[PipelineConfiguration] = PipelineConfiguration()
    subscriptions: List[Topic] = []
    on_update_pipeline: Optional[Callable[[bool], Tuple[bool, Dict[str, str]]]] = None
    on_select_device: Optional[Callable[[str], Tuple[bool, Dict[str, str]]]] = None
    on_reset: Optional[Callable[[], Tuple[bool, Dict[str, str]]]] = None

    def handle_action(self, action: Action, **kwargs) -> Tuple[bool, Dict[str, str]]:  # type: ignore[no-untyped-def]
        if action == Action.UPDATE_PIPELINE:
            if kwargs.get("pipeline_config", None):
                if self.on_update_pipeline:
                    old_pipeline_config = self.pipeline_config
                    self.pipeline_config = kwargs.get("pipeline_config")
                    success, message = self.on_update_pipeline(kwargs.get("runtime_only"))  # type: ignore[arg-type]

                    print(f"Updating pipeline: {'successful' if success else 'failed'} with message: {message}")
                    if success:
                        return success, message
                    self.pipeline_config = old_pipeline_config
                    return success, message
        elif action == Action.SELECT_DEVICE:
            device_id = kwargs.get("device_id", None)
            if device_id is not None:
                self.device_id = device_id
                if self.on_select_device:
                    return self.on_select_device(device_id)
        elif action == Action.GET_SUBSCRIPTIONS:
            return self.subscriptions  # type: ignore[return-value]
        elif action == Action.SET_SUBSCRIPTIONS:
            self.subscriptions = kwargs.get("subscriptions", [])
        elif action == Action.GET_PIPELINE:
            return self.pipeline_config  # type: ignore[return-value]
        elif action == Action.RESET:
            if self.on_reset:
                self.pipeline_config = None
                self.subscriptions = []
                return self.on_reset()
        return False, {"message": f"Action: {action} didn't succeed!"}
