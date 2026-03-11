# method_missing without respond_to_missing?

class Proxy
  def method_missing(name, *args)
    @target.send(name, *args)
  end
end

class DynamicFinder
  def method_missing(method_name, *arguments, &block)
    if method_name.to_s.start_with?("find_by_")
      find(method_name.to_s.sub("find_by_", ""), *arguments)
    else
      super
    end
  end
end
