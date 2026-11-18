// Run the MiST K1801VM1 model against an ideal memory and log the CPU-clock
// count of every write to the probe address 177000.
//   tb PROGRAM.bin DIVIDER DOUT_DELAY [WAIT]
// DIVIDER: system clocks per CPU clock (32 = BK-0010, 24 = BK-0011M).
// DOUT_DELAY: CPU clocks the write strobe is delayed (1 = BK-0010, 3 = BK-0011M).
// WAIT: extra CPU clocks before memory replies (0 = immediate).
#include "Vvm1_se.h"
#include "verilated.h"
#include "Vvm1_se___024root.h"
#include <cstdio>
#include <cstdint>
#include <vector>
#include <cstdlib>

static uint8_t mem[65536];

int main(int argc, char** argv) {
    Verilated::commandArgs(argc, argv);
    setvbuf(stdout, nullptr, _IONBF, 0);
    const uint64_t max_clocks = getenv("MAXCLK") ? strtoull(getenv("MAXCLK"), nullptr, 10) : 50000000ull;
    if (argc < 4) { fprintf(stderr, "usage: tb PROGRAM.bin DIVIDER DOUT_DELAY [WAIT]\n"); return 2; }
    FILE* f = fopen(argv[1], "rb");
    if (!f) { perror(argv[1]); return 1; }
    // IMAGE=1: the file is a raw 64 KB memory image; SEL1=octal sets the
    // start address the CPU reads at reset; DUMP=file saves memory at the end.
    const bool image = getenv("IMAGE") != nullptr;
    const unsigned sel1 = getenv("SEL1") ? strtoul(getenv("SEL1"), nullptr, 8) : 0100300;
    if (image) {
        fread(mem, 1, sizeof mem, f); fclose(f);
    } else {
    uint8_t hdr[4]; fread(hdr, 1, 4, f);
    unsigned load = hdr[0] | hdr[1] << 8, len = hdr[2] | hdr[3] << 8;
    fread(mem + load, 1, len, f); fclose(f);
    if (load != 0100000) {  // start address is 100000: jump to the program
        mem[0100000] = 0137; mem[0100001] = 0; mem[0100002] = load & 0xff; mem[0100003] = load >> 8;
    }
    }
    // MIST=1: RAM below 100000 is paced by the 037 memory controller, as in
    // the MiST core: a phase counter on a 6 MHz clock (96 MHz system clock /
    // 16) gives the CPU one memory slot per 8 phases. ROM/I/O reply at once.
    const bool mist = getenv("MIST") != nullptr;
    int div16 = 0, PC = 0; bool PC90 = false, RASEL = false, TRPLY = false;
    const int divider = atoi(argv[2]), dout_delay = atoi(argv[3]);
    const int wait = argc > 4 ? atoi(argv[4]) : 0;
    // EVNT_AT=addr (octal): raise EVNT (IRQ2) when the CPU reads addr, and
    // drop it when the CPU fetches the EVNT vector (100).
    const long evnt_at = getenv("EVNT_AT") ? strtol(getenv("EVNT_AT"), nullptr, 8) : -1;

    Vvm1_se* top = new Vvm1_se;
    top->pin_clk = 0; top->pin_ce_p = 0; top->pin_ce_n = 0; top->pin_ce_timer = 0;
    top->pin_dclo = 1; top->pin_aclo = 1; top->pin_irq = 0; top->pin_virq = 0;
    top->pin_dmr = 0; top->pin_sack = 0; top->pin_din = 0; top->pin_rply = 0;
    top->pin_din_stb_in = 0; top->pin_dout_stb_in = 0;
    top->eval();

    int cpu_div = 0; bool ce_p = false, ce_n = false, ack = false;
    unsigned dout_shift = 0; int waited = 0;
    uint64_t cpu_clocks = 0;
    for (uint64_t sys = 0; cpu_clocks < max_clocks; sys++) {
        // Reset: hold DCLO, then ACLO, as the reset module does.
        if (cpu_clocks == 20000) top->pin_dclo = 0;
        if (cpu_clocks == 220000) top->pin_aclo = 0;

        // Rising edge: the core samples its inputs.
        top->pin_clk = 1; top->eval();

        // Falling edge: clock enables, write-strobe delay and reply, as in MiST.
        bool ce_bus = ce_p;
        {
            bool stb = top->pin_din_stb_out || top->pin_dout_stb_in;
            bool legacy = (top->pin_addr < 0100000) && !top->pin_sel;
            bool dio = mist && legacy && top->pin_sync && stb;
            div16 = (div16 + 1) & 15;
            bool ce6p = div16 == 0, ce6n = div16 == 8;
            if (ce6n) { int old = PC; PC = (PC + 1) & 7; if (!(old & 1)) PC90 = (old >> 1) & 1; }
            if (ce6p) {
                bool pc1 = (PC >> 1) & 1, pc2 = (PC >> 2) & 1;
                if (PC90 && pc1) RASEL = false;
                else if (PC90 && !pc1 && pc2) RASEL = top->pin_sync && !(TRPLY && !RASEL) && dio;
            }
            if (RASEL) TRPLY = true; else if (!dio) TRPLY = false;
        }
        if (ce_bus) {
            dout_shift = (dout_shift << 1) | (top->pin_dout_stb_out ? 1 : 0);
            bool dout_in = (dout_shift >> (dout_delay - 1)) & 1;
            top->pin_dout_stb_in = dout_in && top->pin_dout_stb_out;
            bool sel = top->pin_sel != 0;
            bool strobe = top->pin_sync && (top->pin_din_stb_out || top->pin_dout_stb_in) && !sel;
            bool legacy_now = mist && top->pin_addr < 0100000;
            if (legacy_now) {
                ack = strobe && TRPLY && !RASEL;
            } else if (strobe && !ack) {
                if (waited < wait) { waited++; } else { ack = true; waited = 0; }
            } else if (!strobe) { ack = false; waited = 0; }
            // Memory: write when the reply goes out for a write cycle.
            uint16_t a = top->pin_addr & 0xfffe;
            if (ack && top->pin_dout_stb_in && !top->pin_sel) {
                if (a == 0177000) {
                    printf("probe %06o at %llu\n", top->pin_dout, (unsigned long long)cpu_clocks);
                    if (top->pin_dout == 0177777) return 0;
                } else {
                    if (top->pin_wtbt & 1) mem[a] = top->pin_dout & 0xff;
                    if (top->pin_wtbt & 2) mem[a + 1] = top->pin_dout >> 8;
                    if (!(top->pin_wtbt & 3)) { mem[a] = top->pin_dout & 0xff; mem[a + 1] = top->pin_dout >> 8; }
                }
            }
            if (ack && top->pin_din_stb_out && !top->pin_sel) {
                if ((long)a == evnt_at) top->pin_irq |= 2;
                if (a == 0100 && evnt_at >= 0) top->pin_irq &= ~2;
            }
            top->pin_rply = ack;
        }
        if (getenv("TRACE") && cpu_clocks < (uint64_t)atoi(getenv("TRACE")) && ce_bus) {
            static unsigned last = 0xffffffff;
            unsigned st = top->pin_sync | top->pin_din_stb_out << 1 | top->pin_dout_stb_out << 2 | top->pin_rply << 3 | (unsigned)top->pin_sel << 4 | top->pin_dclo << 6 | top->pin_aclo << 7;
            unsigned key = st << 16 | top->pin_addr;
            if (key != last) { last = key; printf("clk %4llu addr %06o sync %d din %d dout %d rply %d sel %d dclo %d aclo %d init %d\n", (unsigned long long)cpu_clocks, top->pin_addr, top->pin_sync, top->pin_din_stb_out, top->pin_dout_stb_out, top->pin_rply, top->pin_sel, top->pin_dclo, top->pin_aclo, top->pin_init); }
        }
        if (getenv("INNER") && ce_bus) {
            uint64_t from = strtoull(getenv("INNER"), nullptr, 10);
            if (cpu_clocks >= from && cpu_clocks < from + 40) {
                auto* r = top->rootp;
                printf("clk %llu reset %d mjres %d abort %d aclo %d acok %d plm_stb %d mj %05o plir %o rq %06o qreq %d sync %d | uop %d oat %d tovf %d plm_ena %d ir %06o plr12 %d\n",
                    (unsigned long long)cpu_clocks, r->vm1_se__DOT__core__DOT__reset, r->vm1_se__DOT__core__DOT__mjres,
                    r->vm1_se__DOT__core__DOT__abort, r->vm1_se__DOT__core__DOT__aclo, r->vm1_se__DOT__core__DOT__acok,
                    r->vm1_se__DOT__core__DOT__plm_stb_rc, (unsigned)r->vm1_se__DOT__core__DOT__mj, (unsigned)r->vm1_se__DOT__core__DOT__plir,
                    (unsigned)r->vm1_se__DOT__core__DOT__rq, r->vm1_se__DOT__core__DOT__qbus_req, top->pin_sync,
                    r->vm1_se__DOT__core__DOT__exc_uop, r->vm1_se__DOT__core__DOT__exc_oat, r->vm1_se__DOT__core__DOT__qbus_tovf,
                    r->vm1_se__DOT__core__DOT__plm_ena, (unsigned)r->vm1_se__DOT__core__DOT__ir, (int)((r->vm1_se__DOT__core__DOT__plr >> 12) & 1));
            }
        }
        // Data in: the latched address's word, or SEL1 (start address 100000).
        uint16_t a = top->pin_addr & 0xfffe;
        top->pin_din = (top->pin_sel & 1) ? sel1 : (mem[a] | mem[a + 1] << 8);
        top->pin_din_stb_in = top->pin_din_stb_out;
        cpu_div = (cpu_div + 1 == divider) ? 0 : cpu_div + 1;
        ce_p = cpu_div == 0;
        ce_n = cpu_div == divider / 2;
        if (ce_p) cpu_clocks++;
        top->pin_ce_p = ce_p; top->pin_ce_n = ce_n; top->pin_ce_timer = ce_p;
        top->pin_clk = 0; top->eval();
    }
    if (getenv("DUMP")) { FILE* d = fopen(getenv("DUMP"), "wb"); fwrite(mem, 1, sizeof mem, d); fclose(d); }
    fprintf(stderr, "timeout\n");
    return 1;
}
