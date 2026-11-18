// Behavioural stand-in for the Altera dual-port RAM used as the VM1 model's
// register file: 64 x 16, registered addresses, unregistered outputs, byte
// enables on port A, initial contents from the core's .mif file.
module vm1_aram (
    input  [5:0]  address_a,
    input  [5:0]  address_b,
    input  [1:0]  byteena_a,
    input         clock,
    input  [15:0] data_a,
    input  [15:0] data_b,
    input         wren_a,
    input         wren_b,
    output [15:0] q_a,
    output [15:0] q_b
);
    reg [15:0] mem [0:63];
    reg [5:0] ra, rb;
    initial $readmemh("regfile.hex", mem);
    always @(posedge clock) begin
        ra <= address_a;
        rb <= address_b;
        if (wren_a) begin
            if (byteena_a[0]) mem[address_a][7:0]  <= data_a[7:0];
            if (byteena_a[1]) mem[address_a][15:8] <= data_a[15:8];
        end
        if (wren_b) mem[address_b] <= data_b;
    end
    assign q_a = mem[ra];
    assign q_b = mem[rb];
endmodule
