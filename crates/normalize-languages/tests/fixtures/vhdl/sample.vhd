library ieee;
use ieee.std_logic_1164.all;
use ieee.numeric_std.all;

-- Simple FIFO entity
entity fifo is
    generic (
        DATA_WIDTH : integer := 8;
        DEPTH      : integer := 16
    );
    port (
        clk      : in  std_logic;
        rst      : in  std_logic;
        wr_en    : in  std_logic;
        rd_en    : in  std_logic;
        data_in  : in  std_logic_vector(DATA_WIDTH - 1 downto 0);
        data_out : out std_logic_vector(DATA_WIDTH - 1 downto 0);
        full     : out std_logic;
        empty    : out std_logic
    );
end entity fifo;

architecture rtl of fifo is
    type mem_type is array (0 to DEPTH - 1) of
        std_logic_vector(DATA_WIDTH - 1 downto 0);

    signal mem       : mem_type;
    signal wr_ptr    : unsigned(3 downto 0) := (others => '0');
    signal rd_ptr    : unsigned(3 downto 0) := (others => '0');
    signal count     : unsigned(4 downto 0) := (others => '0');
begin

    full  <= '1' when count = DEPTH else '0';
    empty <= '1' when count = 0    else '0';

    process(clk) is
    begin
        if rising_edge(clk) then
            if rst = '1' then
                wr_ptr <= (others => '0');
                rd_ptr <= (others => '0');
                count  <= (others => '0');
            else
                if wr_en = '1' and count < DEPTH then
                    mem(to_integer(wr_ptr)) <= data_in;
                    wr_ptr <= wr_ptr + 1;
                    count  <= count + 1;
                end if;
                if rd_en = '1' and count > 0 then
                    data_out <= mem(to_integer(rd_ptr));
                    rd_ptr <= rd_ptr + 1;
                    count  <= count - 1;
                end if;
            end if;
        end if;
    end process;

end architecture rtl;

package fifo_pkg is
    component fifo is
        generic (
            DATA_WIDTH : integer := 8;
            DEPTH      : integer := 16
        );
        port (
            clk      : in  std_logic;
            rst      : in  std_logic;
            wr_en    : in  std_logic;
            rd_en    : in  std_logic;
            data_in  : in  std_logic_vector(DATA_WIDTH - 1 downto 0);
            data_out : out std_logic_vector(DATA_WIDTH - 1 downto 0);
            full     : out std_logic;
            empty    : out std_logic
        );
    end component fifo;
end package fifo_pkg;
