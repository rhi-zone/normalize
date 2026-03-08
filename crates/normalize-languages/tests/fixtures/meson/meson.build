project('mylib', 'c', 'cpp',
  version: '1.2.0',
  license: 'MIT',
  default_options: ['c_std=c11', 'cpp_std=c++17', 'warning_level=2'],
)

glib_dep = dependency('glib-2.0', version: '>= 2.56')
zlib_dep = dependency('zlib')
thread_dep = dependency('threads')

protobuf_proj = subproject('protobuf')
protobuf_dep = protobuf_proj.get_variable('protobuf_dep')

cc = meson.get_compiler('c')
m_dep = cc.find_library('m', required: false)

conf_data = configuration_data()
conf_data.set('VERSION', meson.project_version())
conf_data.set10('HAVE_THREADS', thread_dep.found())
configure_file(
  input: 'config.h.in',
  output: 'config.h',
  configuration: conf_data,
)

lib_sources = files(
  'src/core.c',
  'src/util.c',
  'src/parser.c',
)

mylib = library('mylib',
  lib_sources,
  dependencies: [glib_dep, zlib_dep, thread_dep],
  include_directories: include_directories('include'),
  version: meson.project_version(),
  install: true,
)

mylib_dep = declare_dependency(
  link_with: mylib,
  include_directories: include_directories('include'),
  dependencies: [glib_dep],
)

executable('myapp',
  'src/main.c',
  dependencies: [mylib_dep, m_dep],
  install: true,
)

if get_option('tests')
  subdir('tests')
endif
