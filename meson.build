project(
  'pigoune',
  version: '1.6.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.5.0',
)

i18n = import('i18n')

cargo = find_program('cargo')

if get_option('buildtype') == 'release'
  cargo_profile = 'release'
  cargo_profile_dir = 'release'
else
  cargo_profile = 'dev'
  cargo_profile_dir = 'debug'
endif

cargo_target_dir = meson.project_build_root() / 'cargo-target'

custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  console: true,
  output: meson.project_name(),
  install: true,
  install_dir: get_option('bindir'),
  env: {
    'PIGOUNE_LOCALEDIR': get_option('prefix') / get_option('localedir'),
  },
  command: [
    cargo, 'build',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    '--target-dir', cargo_target_dir,
    '--profile', cargo_profile,
    '--package', 'pigoune-app',
    '&&',
    'cp', cargo_target_dir / cargo_profile_dir / meson.project_name(), '@OUTPUT@',
  ],
)

subdir('data')
subdir('po')

gnome = import('gnome')
gnome.post_install(glib_compile_schemas: true)
