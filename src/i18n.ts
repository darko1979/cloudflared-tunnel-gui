import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from './locales/en.json';
import zh from './locales/zh.json';
import zhTW from './locales/zh-TW.json';
import es from './locales/es.json';
import ja from './locales/ja.json';
import ptBR from './locales/pt-BR.json';
import de from './locales/de.json';
import fr from './locales/fr.json';
import ru from './locales/ru.json';
import ko from './locales/ko.json';
import hr from './locales/hr.json';
import srLatn from './locales/sr-Latn.json';

// Shown in Settings under each language's own name
export const languages = [
    { code: 'en', label: 'English', translation: en },
    { code: 'zh', label: '简体中文', translation: zh },
    { code: 'zh-TW', label: '繁體中文', translation: zhTW },
    { code: 'es', label: 'Español', translation: es },
    { code: 'ja', label: '日本語', translation: ja },
    { code: 'pt-BR', label: 'Português (Brasil)', translation: ptBR },
    { code: 'de', label: 'Deutsch', translation: de },
    { code: 'fr', label: 'Français', translation: fr },
    { code: 'ru', label: 'Русский', translation: ru },
    { code: 'ko', label: '한국어', translation: ko },
    { code: 'hr', label: 'Hrvatski', translation: hr },
    { code: 'sr-Latn', label: 'Srpski', translation: srLatn },
];

const resources = Object.fromEntries(languages.map(({ code, translation }) => [code, { translation }]));

i18n.use(initReactI18next).init({
    resources,
    lng: localStorage.getItem('language') || 'en',
    fallbackLng: 'en',
    interpolation: {
        escapeValue: false,
    },
});

export default i18n;
