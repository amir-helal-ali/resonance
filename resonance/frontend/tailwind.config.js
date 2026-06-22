/** @type {import('tailwindcss').Config} */
export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],
  theme: {
    extend: {
      colors: {
        // ---- Resonance Identity: Egyptian-Inspired Palette ----
        papyrus: {
          50:  '#FBF8F1', // off-white "رمال النيل"
          100: '#F5EFE0',
          200: '#EBE0C5',
          300: '#DCC9A0',
          400: '#C9AE76',
          500: '#B89455',
          600: '#9A7740',
          700: '#7B5C32',
          800: '#5C4425',
          900: '#3E2D19',
        },
        'astral-blue': {
          50:  '#1A2240', // "سقف الأقصر الليلي" — Deep Astral Blue
          100: '#161D38',
          200: '#121A30',
          300: '#0E1628',
          400: '#0A1220',
          500: '#070E18',
          600: '#050A12',
          700: '#03070C',
          800: '#020508',
          900: '#010304',
        },
        'egyptian-gold': {
          50:  '#FFF7E0',
          100: '#FFE9A8',
          200: '#FBD876',
          300: '#F4C44A',
          400: '#E5A91E', // primary accent "ذهبي مصري"
          500: '#C48B14',
          600: '#9E6E0C',
          700: '#785407',
          800: '#523B04',
          900: '#2C2002',
        },
      },
      fontFamily: {
        // The Breathing Interface uses `Cairo` for UI and `Amiri` for manifesto.
        sans: ['Cairo', 'system-ui', 'sans-serif'],
        serif: ['Amiri', 'Georgia', 'serif'],
      },
      animation: {
        'sun-cycle': 'sun-cycle 60s linear infinite',
        'nile-flow': 'nile-flow 0.6s ease-out',
        'aura-pulse': 'aura-pulse 2s ease-in-out infinite',
      },
      keyframes: {
        'sun-cycle': {
          '0%':   { backgroundPosition: '0% 50%' },
          '50%':  { backgroundPosition: '100% 50%' },
          '100%': { backgroundPosition: '0% 50%' },
        },
        'nile-flow': {
          '0%':   { transform: 'translateY(8px)', opacity: '0' },
          '100%': { transform: 'translateY(0)',  opacity: '1' },
        },
        'aura-pulse': {
          '0%, 100%': { opacity: '0.6', transform: 'scale(1)' },
          '50%':      { opacity: '1',   transform: 'scale(1.06)' },
        },
      },
    },
  },
  plugins: [],
};
