import { test } from 'node:test';
import assert from 'node:assert/strict';
import { search, distance, fold } from '../src/lib/site-search.ts';

const idx = [
  { id: 'secure.password-generator', name: 'Générateur de mots de passe', tagline: 'Des mots de passe solides', keywords: ['mot de passe', 'password', 'aléatoire'], intents: ['créer un mot de passe sécurisé'], path: '/p', glyph: '', variants: [{ name: 'Code PIN aléatoire', path: '/p/pin' }] },
  { id: 'pdf.merge', name: 'Fusionner des PDF', tagline: 'Assemblez plusieurs PDF', keywords: ['fusionner pdf', 'assembler pdf', 'joindre pdf'], intents: ['fusionner plusieurs pdf en un seul'], path: '/m', glyph: '', variants: [] },
  { id: 'image.convert', name: "Convertisseur d'images", tagline: 'PNG, JPG, WebP', keywords: ['convertir image', 'png en jpg', 'webp'], intents: ['convertir une image png en jpg'], path: '/i', glyph: '', variants: [{ name: 'Convertir PNG en JPG', path: '/i/png-jpg' }, { name: 'Convertir WebP en PNG', path: '/i/webp-png' }] },
  { id: 'calc.gross-net-salary', name: 'Salaire brut en net', tagline: 'Estimez votre net', keywords: ['salaire brut net', 'fiche de paie'], intents: ['calculer mon salaire net'], path: '/s', glyph: '', variants: [] },
  { id: 'qr.generator', name: 'Générateur de QR code', tagline: 'QR codes nets', keywords: ['qr code', 'lien'], intents: ['mettre un lien dans un qr code'], path: '/q', glyph: '', variants: [] },
];
const top = (q) => search(idx, q)[0]?.entry.id;

test('folds accents and punctuation', () => {
  assert.equal(fold("Générateur d'ÉTÉ!"), 'generateur d ete');
});

test('edit distance counts transpositions and is bounded', () => {
  assert.equal(distance('pdf', 'pdf'), 0);
  assert.equal(distance('fusionner', 'fusionenr'), 1);
  assert.ok(distance('abcdefgh', 'zzzzzzzz', 2) > 2);
});

test('direct keyword and name queries', () => {
  assert.equal(top('pdf'), 'pdf.merge');
  assert.equal(top('mot de passe'), 'secure.password-generator');
  assert.equal(top('qr'), 'qr.generator');
});

test('typos and missing accents still match', () => {
  assert.equal(top('fusionenr pdf'), 'pdf.merge');
  assert.equal(top('generateur mot de pasee'), 'secure.password-generator');
  assert.equal(top('convertissur'), 'image.convert');
});

test('natural-language intents and synonyms', () => {
  assert.equal(top('transformer une photo en jpg'), 'image.convert');
  assert.equal(top('assembler des documents pdf'), 'pdf.merge');
  assert.equal(top('mettre un lien dans un qr code'), 'qr.generator');
  assert.equal(top('calculer mon salaire net'), 'calc.gross-net-salary');
  assert.equal(top('mdp'), 'secure.password-generator');
});

test('variants are surfaced when the query names them', () => {
  const r = search(idx, 'png en jpg')[0];
  assert.equal(r.entry.id, 'image.convert');
  assert.equal(r.variant?.name, 'Convertir PNG en JPG');
});

test('empty or unrelated queries return nothing', () => {
  assert.deepEqual(search(idx, ''), []);
  assert.deepEqual(search(idx, 'zzzzqqq'), []);
});

test('short queries land on the tool, not on a variant', () => {
  const r = search(idx, 'mdp')[0];
  assert.equal(r.entry.id, 'secure.password-generator');
  assert.equal(r.variant, undefined);
});

test('a single weak shared word does not drag unrelated tools in', () => {
  const ids = search(idx, 'calculer mon salaire net').map((r) => r.entry.id);
  assert.deepEqual(ids, ['calc.gross-net-salary']);
});
