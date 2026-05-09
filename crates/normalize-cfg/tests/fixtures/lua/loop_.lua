function loop_(n)
    local sum = 0
    for i = 1, n do
        if i == 5 then
            break
        end
        sum = sum + i
    end
    return sum
end
