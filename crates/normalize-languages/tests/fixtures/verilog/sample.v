// ALU module with basic arithmetic and logic operations
module alu #(
    parameter WIDTH = 32
) (
    input  wire             clk,
    input  wire             rst_n,
    input  wire [WIDTH-1:0] a,
    input  wire [WIDTH-1:0] b,
    input  wire [3:0]       op,
    output reg  [WIDTH-1:0] result,
    output reg              zero,
    output reg              overflow
);

    localparam OP_ADD  = 4'h0;
    localparam OP_SUB  = 4'h1;
    localparam OP_AND  = 4'h2;
    localparam OP_OR   = 4'h3;
    localparam OP_XOR  = 4'h4;
    localparam OP_SHL  = 4'h5;
    localparam OP_SHR  = 4'h6;

    always @(posedge clk or negedge rst_n) begin
        if (!rst_n) begin
            result   <= {WIDTH{1'b0}};
            zero     <= 1'b0;
            overflow <= 1'b0;
        end else begin
            case (op)
                OP_ADD: result <= a + b;
                OP_SUB: result <= a - b;
                OP_AND: result <= a & b;
                OP_OR:  result <= a | b;
                OP_XOR: result <= a ^ b;
                OP_SHL: result <= a << b[4:0];
                OP_SHR: result <= a >> b[4:0];
                default: result <= {WIDTH{1'b0}};
            endcase
            zero <= (result == {WIDTH{1'b0}});
        end
    end

    assign overflow = (op == OP_ADD) && (a[WIDTH-1] == b[WIDTH-1]) &&
                      (result[WIDTH-1] != a[WIDTH-1]);

endmodule

// Register file module
module reg_file #(
    parameter WIDTH = 32,
    parameter DEPTH = 32
) (
    input  wire             clk,
    input  wire             we,
    input  wire [$clog2(DEPTH)-1:0] raddr1,
    input  wire [$clog2(DEPTH)-1:0] raddr2,
    input  wire [$clog2(DEPTH)-1:0] waddr,
    input  wire [WIDTH-1:0] wdata,
    output wire [WIDTH-1:0] rdata1,
    output wire [WIDTH-1:0] rdata2
);
    reg [WIDTH-1:0] regs [0:DEPTH-1];

    assign rdata1 = regs[raddr1];
    assign rdata2 = regs[raddr2];

    always @(posedge clk) begin
        if (we && waddr != 0)
            regs[waddr] <= wdata;
    end
endmodule
