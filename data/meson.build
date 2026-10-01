app_id = 'io.github.gor3pig.Pigoune'

install_data(
  app_id + '.desktop',
  install_dir: get_option('datadir') / 'applications',
)

install_data(
  app_id + '.metainfo.xml',
  install_dir: get_option('datadir') / 'metainfo',
)

foreach size : ['16', '32', '48', '64', '128', '256', '512']
  install_data(
    'icons' / 'pigoune-' + size + 'x' + size + '.png',
    install_dir: get_option('datadir') / 'icons' / 'hicolor' / size + 'x' + size / 'apps',
    rename: app_id + '.png',
  )
endforeach
