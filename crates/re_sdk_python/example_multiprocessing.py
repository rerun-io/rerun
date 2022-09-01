import rerun_sdk as rerun
from multiprocessing import Process

def task(title):
    # This should connect using the same recording id
    rerun.connect()
    rerun.log_rect(title, [10, 20], [30, 40], label=title, space=title)

if __name__ == '__main__':
    task('main_task')
    p = Process(target=task, args=('chiild_task',))
    p.start()
    p.join()
