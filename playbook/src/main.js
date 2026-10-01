import { createApp } from 'vue'
import { Quasar } from 'quasar'
// Quasar's base stylesheet first, so the playbook's own style.css (imported after) wins any overlap.
import 'quasar/dist/quasar.prod.css'
import App from './App.vue'
import './style.css'

createApp(App).use(Quasar, { config: { dark: true } }).mount('#app')
