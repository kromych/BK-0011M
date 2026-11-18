# BK documentation

Guides to the Elektronika BK-0010, BK-0010.01 and BK-0011M computers and to
this emulator of them. They are based on the original Soviet manuals (in
[manuals/](manuals/), in Russian), with every example checked in the
emulator.

**Getting started**

- [emulator.md](emulator.md): build and run the emulator; choose a model;
  the keyboard; load programs, tapes and disks; save states; the debugger;
  scripted runs.
- [machine.md](machine.md): what the computers are: the models, the
  hardware, memory maps, the screen, and the software in their ROMs.

**Using the computer**

- [monitor.md](monitor.md): the system monitors (BK-0010 monitor, test
  monitor, BK-0011M keyboard monitor and МСД), loading software from tape and
  disk, and the keyboard and screen as the user meets them.

**Programming**

- [basic.md](basic.md): BASIC on the BK-0010.01 and the BK-0011M.
- [focal.md](focal.md): FOCAL on the BK-0010.
- [machine-code.md](machine-code.md): the K1801VM1 processor, its
  instructions, the project's assembler, first programs, debugging.
- [system-programming.md](system-programming.md): the monitor's and BOS's
  system calls, interrupts, devices, memory paging, tape and disk formats.

**Reference**

- [hardware-reference.md](hardware-reference.md): the hardware at register
  level, as the emulator implements it, with sources.
- [manuals/](manuals/README.md): the original manuals.

Numbers that are addresses or machine words are octal throughout, as in the
original documentation.
