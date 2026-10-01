app_id = 'io.github.gor3pig.Pigoune'

i18n.merge_file(
  input: app_id + '.desktop.in',
  output: app_id + '.desktop',
  type: 'desktop',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'applications',
)

i18n.merge_file(
  input: app_id + '.metainfo.xml.in',
  output: app_id + '.metainfo.xml',
  po_dir: '../po',
  install: true,
  install_dir: get_option('datadir') / 'metainfo',
)

foreach size : ['16', '32', '48', '64', '128', '256', '512']
  install_data(
    'icons' / 'pigoune-' + size + 'x' + size + '.png',
    install_dir: get_option('datadir') / 'icons' / 'hicolor' / size + 'x' + size / 'apps',
    rename: app_id + '.png',
  )
endforeach

install_data(
  app_id + '.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)
