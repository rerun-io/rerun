import rerun_sdk as rerun
import multiprocessing

def task(title):
    # All processes spawned with `multiprocessing` will automatically
    # be assigned the same default recording_id.
    print(f'Logging using the rerun recording id {rerun.get_recording_id()}');
    rerun.connect()
    rerun.log_rect(title, [10, 20], [30, 40], label=title, space=title)

if __name__ == '__main__':
    task('main_task')
    p = multiprocessing.Process(target=task, args=('child_task',))
    p.start()
    p.join()
