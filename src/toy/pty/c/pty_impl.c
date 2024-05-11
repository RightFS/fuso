#include <errno.h>
#include <fcntl.h>
#include <pty.h>
#include <signal.h>
#include <stdio.h>
#include <sys/wait.h>
#include <unistd.h>
#include <wait.h>

#include "pty_impl.h"

void enter_pty_main(const int argc, char *const *argv, const char **envp);

int pty_spawn(pty_process *process) {
  int pid;
  int master;
  int flags;
  int status;

  struct winsize size = {.ws_row = 100, .ws_col = 100, 0, 0};

  pid = forkpty(&master, NULL, NULL, &size);

  if (pid < 0) {
    return -errno;
  } else if (pid == 0) {
    enter_pty_main(process->argc, process->argv, process->envp);
  }

  if ((flags = fcntl(master, F_GETFL)) == -1) {
    status = -errno;
    goto exit_pty;
  }

  if (fcntl(master, F_SETFL, flags | O_NONBLOCK) == -1) {
    status = -errno;
    goto exit_pty;
  }

  process->pid = pid;
  process->pty = master;

  return 0;

exit_pty:

  close(master);

  kill(pid, SIGKILL);

  waitpid(pid, NULL, 0);

  return status;
}

void pty_exit(pty_process *process) {
  if (!process)
    return;

  if (process->pty) {
    close(process->pty);
  }

  kill(process->pid, SIGKILL);
  waitpid(process->pid, NULL, 0);

  process->pty = -1;
  process->pid = -1;
}

void enter_pty_main(const int argc, char *const *argv, const char **envp) {
  int ret = execvp(argv[0], argv);
  if (ret < 0) {
    _exit(-errno);
  }
}
