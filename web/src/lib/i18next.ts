import i18next from 'i18next';
import * as en from '../../locales/en.json';
import * as de from '../../locales/de.json';

i18next.init({
  resources: {
    en: {
      translation: en,
    },
    de: {
      translation: de,
    },
  },
  lng: 'en',
});

const intl = i18next.t;

export { i18next, intl };
