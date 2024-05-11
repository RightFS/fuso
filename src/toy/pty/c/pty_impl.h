#ifndef __TTY_H__
#define __TTY_H__

typedef struct _pty_process {
  int pid;
  int pty;
  const int argc;
  const char* work;
  char *const *argv;
  const char **envp;
} pty_process;

int pty_spawn(pty_process *process);

void pty_exit(pty_process *process);

#endif